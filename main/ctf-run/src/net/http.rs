//! Blocking HTTP/1.1 client: keep-alive TCP with TCP_NODELAY/TCP_QUICKACK, every request sent
//! in one write, `Accept-Encoding: gzip` with transparent decoding, and per-request timings.
//! `http://` always; `https://` with `--features tls`.

use crate::formats::inflate;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_HEAD: usize = 64 * 1024;
const MAX_BODY: usize = 1 << 30;

fn invalid(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Asks the kernel to ACK immediately instead of delaying up to ~40 ms (Linux; not sticky, so
/// re-armed after every write and read). Best effort.
#[cfg(target_os = "linux")]
fn quickack(stream: &TcpStream) {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn setsockopt(fd: i32, level: i32, name: i32, value: *const i32, len: u32) -> i32;
    }
    let on: i32 = 1;
    // SAFETY: valid fd for the duration of the call; `value` points at an i32 of the stated size.
    unsafe {
        setsockopt(stream.as_raw_fd(), 6 /* IPPROTO_TCP */, 12 /* TCP_QUICKACK */, &on, 4);
    }
}

#[cfg(not(target_os = "linux"))]
fn quickack(_: &TcpStream) {}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// Decoded body (gzip already removed).
    pub body: Vec<u8>,
    /// Bytes received on the wire for the body.
    pub wire_len: usize,
    /// Time from the request write to the end of the response head.
    pub ttfb: Duration,
    pub total: Duration,
    /// Whether this went over a kept-alive connection.
    pub reused: bool,
}

impl Response {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Where requests go. Parsed once, shared by every connection.
pub struct Endpoint {
    addr: SocketAddr,
    host: String,
    path: String,
    /// Complete `Authorization: ...\r\n` line, or empty. Only ever set for https.
    auth: String,
    pub https: bool,
    #[cfg(feature = "tls")]
    tls: Option<tls::Config>,
}

impl Endpoint {
    /// `url` is `http://host[:port][/path]` or `https://...`. `token`, if given, is sent as a bearer
    /// token, but only over https: it never goes out in clear text.
    pub fn parse(url: &str, token: Option<&str>) -> io::Result<Arc<Endpoint>> {
        let (https, rest) = if let Some(r) = url.strip_prefix("https://") {
            (true, r)
        } else if let Some(r) = url.strip_prefix("http://") {
            (false, r)
        } else {
            return Err(invalid("the URL must start with http:// or https://"));
        };
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let (name, port) = match authority.rsplit_once(':') {
            Some((n, p)) if p.parse::<u16>().is_ok() => (n, p.parse().unwrap()),
            _ => (authority, if https { 443 } else { 80 }),
        };
        let name = name.trim_start_matches('[').trim_end_matches(']');
        let addr = (name, port).to_socket_addrs()?.next().ok_or_else(|| invalid("host did not resolve"))?;
        let auth = match token {
            Some(t) if https => {
                if t.is_empty() || t.bytes().any(|b| b.is_ascii_control()) {
                    return Err(invalid("the bearer token is empty or contains control characters"));
                }
                format!("Authorization: Bearer {t}\r\n")
            }
            _ => String::new(),
        };
        #[cfg(not(feature = "tls"))]
        if https {
            return Err(invalid("https:// needs a build with `--features tls`"));
        }
        Ok(Arc::new(Endpoint {
            addr,
            host: authority.to_string(),
            path: path.to_string(),
            auth,
            https,
            #[cfg(feature = "tls")]
            tls: if https { Some(tls::Config::new(name)?) } else { None },
        }))
    }

    /// Request target: the base path with `query` (no leading `?`) appended.
    pub fn target(&self, query: &str) -> String {
        if query.is_empty() {
            return self.path.clone();
        }
        let sep = if self.path.contains('?') { '&' } else { '?' };
        format!("{}{sep}{query}", self.path)
    }

    pub fn has_auth(&self) -> bool {
        !self.auth.is_empty()
    }
}

#[cfg(feature = "tls")]
mod tls {
    use rustls::pki_types::{CertificateDer, ServerName, pem::PemObject};
    use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
    use std::io;
    use std::net::TcpStream;
    use std::sync::Arc;

    pub type Stream = StreamOwned<ClientConnection, TcpStream>;

    /// TLS settings for one host; shared so later connections resume the first one's session.
    pub struct Config {
        config: Arc<ClientConfig>,
        name: ServerName<'static>,
    }

    impl Config {
        /// Mozilla roots plus any PEM certificates in `$CTF_CA_FILE`.
        pub fn new(host: &str) -> io::Result<Config> {
            let mut roots = RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
            if let Some(path) = std::env::var_os("CTF_CA_FILE") {
                for cert in CertificateDer::pem_file_iter(&path).map_err(io::Error::other)? {
                    roots.add(cert.map_err(io::Error::other)?).map_err(io::Error::other)?;
                }
            }
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let mut config = ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .map_err(io::Error::other)?
                .with_root_certificates(roots)
                .with_no_client_auth();
            config.alpn_protocols = vec![b"http/1.1".to_vec()];
            let name = ServerName::try_from(host.to_string()).map_err(io::Error::other)?;
            Ok(Config { config: Arc::new(config), name })
        }

        pub fn connect(&self, mut tcp: TcpStream) -> io::Result<Stream> {
            let mut conn =
                ClientConnection::new(self.config.clone(), self.name.clone()).map_err(io::Error::other)?;
            while conn.is_handshaking() {
                conn.complete_io(&mut tcp)?;
            }
            Ok(StreamOwned::new(conn, tcp))
        }
    }
}

enum Stream {
    Plain(TcpStream),
    #[cfg(feature = "tls")]
    Tls(Box<tls::Stream>),
}

impl Stream {
    fn tcp(&self) -> &TcpStream {
        match self {
            Stream::Plain(s) => s,
            #[cfg(feature = "tls")]
            Stream::Tls(s) => &s.sock,
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            #[cfg(feature = "tls")]
            Stream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            #[cfg(feature = "tls")]
            Stream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            #[cfg(feature = "tls")]
            Stream::Tls(s) => s.flush(),
        }
    }
}

/// One keep-alive connection.
pub struct Client {
    ep: Arc<Endpoint>,
    stream: Option<Stream>,
    buf: Vec<u8>,
    pos: usize,
    got_bytes: bool,
    /// Connections opened by this client so far.
    pub connects: u32,
}

impl Client {
    /// A client that connects lazily on its first request.
    pub fn idle(ep: &Arc<Endpoint>) -> Client {
        Client { ep: ep.clone(), stream: None, buf: Vec::with_capacity(32 * 1024), pos: 0, got_bytes: false, connects: 0 }
    }

    /// Connects immediately (including the TLS handshake for https).
    pub fn connected(ep: &Arc<Endpoint>) -> io::Result<Client> {
        let mut c = Client::idle(ep);
        c.connect()?;
        Ok(c)
    }

    pub fn is_open(&self) -> bool {
        self.stream.is_some()
    }

    fn connect(&mut self) -> io::Result<()> {
        let s = TcpStream::connect_timeout(&self.ep.addr, CONNECT_TIMEOUT)?;
        s.set_nodelay(true)?;
        s.set_read_timeout(Some(IO_TIMEOUT))?;
        s.set_write_timeout(Some(IO_TIMEOUT))?;
        #[cfg(feature = "tls")]
        let stream = match &self.ep.tls {
            Some(t) => Stream::Tls(Box::new(t.connect(s)?)),
            None => Stream::Plain(s),
        };
        #[cfg(not(feature = "tls"))]
        let stream = Stream::Plain(s);
        self.stream = Some(stream);
        self.connects += 1;
        Ok(())
    }

    /// `GET <base>?<query>`, asking for gzip (JSON API calls).
    pub fn get(&mut self, query: &str) -> io::Result<Response> {
        let target = self.ep.target(query);
        let req = self.head("GET", &target, None, true);
        self.send(&req)
    }

    /// `GET <base>?<query>` without `Accept-Encoding`: for artifact downloads, which are already
    /// compressed (zip/apk) - gzip would add server work, wire bytes and a decode pass.
    pub fn get_identity(&mut self, query: &str) -> io::Result<Response> {
        let target = self.ep.target(query);
        let req = self.head("GET", &target, None, false);
        self.send(&req)
    }

    /// `POST <base>` with a JSON body.
    pub fn post_json(&mut self, body: &str) -> io::Result<Response> {
        let target = self.ep.target("");
        let mut req = self.head("POST", &target, Some(body.len()), true);
        req.extend_from_slice(body.as_bytes());
        self.send(&req)
    }

    fn head(&self, method: &str, target: &str, body_len: Option<usize>, gzip: bool) -> Vec<u8> {
        let mut req = Vec::with_capacity(256 + body_len.unwrap_or(0));
        let _ = write!(req, "{method} {target} HTTP/1.1\r\nHost: {}\r\n{}", self.ep.host, self.ep.auth);
        if gzip {
            req.extend_from_slice(b"Accept-Encoding: gzip\r\n");
        }
        if let Some(n) = body_len {
            let _ = write!(req, "Content-Type: application/json\r\nContent-Length: {n}\r\n");
        }
        req.extend_from_slice(b"\r\n");
        req
    }

    fn send(&mut self, req: &[u8]) -> io::Result<Response> {
        let reused = self.stream.is_some();
        match self.round_trip(req, reused) {
            Ok(r) => Ok(r),
            Err(e) => {
                self.stream = None;
                // A kept-alive connection the server closed while idle fails before any response
                // byte arrives; only that case is retried, once, on a fresh connection.
                if reused && !self.got_bytes {
                    let r = self.round_trip(req, false);
                    if r.is_err() {
                        self.stream = None;
                    }
                    r
                } else {
                    Err(e)
                }
            }
        }
    }

    fn round_trip(&mut self, req: &[u8], reused: bool) -> io::Result<Response> {
        self.buf.clear();
        self.pos = 0;
        self.got_bytes = false;
        if self.stream.is_none() {
            self.connect()?;
        }
        let t0 = Instant::now();
        let stream = self.stream.as_mut().unwrap();
        stream.write_all(req)?;
        stream.flush()?;
        quickack(stream.tcp());

        let (status, head) = loop {
            let end = loop {
                if let Some(i) = find(&self.buf[self.pos..], b"\r\n\r\n") {
                    break self.pos + i;
                }
                if self.buf.len() - self.pos > MAX_HEAD {
                    return Err(invalid("response head too large"));
                }
                self.fill()?;
            };
            let head = String::from_utf8_lossy(&self.buf[self.pos..end]).into_owned();
            self.pos = end + 4;
            let status: u16 = head
                .split(' ')
                .nth(1)
                .and_then(|s| s.get(..3))
                .and_then(|s| s.parse().ok())
                .filter(|_| head.starts_with("HTTP/1."))
                .ok_or_else(|| invalid("malformed status line"))?;
            if !(100..200).contains(&status) {
                break (status, head);
            }
        };
        let ttfb = t0.elapsed();
        let headers: Vec<(String, String)> = head
            .split("\r\n")
            .skip(1)
            .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
            .collect();
        let hv = |name: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str());
        let mut close = head.starts_with("HTTP/1.0") || hv("connection").is_some_and(|v| v.eq_ignore_ascii_case("close"));
        let chunked = hv("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));
        let length = match hv("content-length") {
            Some(v) => Some(v.parse::<usize>().map_err(|_| invalid("bad content-length"))?),
            None => None,
        };
        let gzip = hv("content-encoding").is_some_and(|v| v.eq_ignore_ascii_case("gzip"));

        let mut body = Vec::with_capacity(length.unwrap_or(16 * 1024).min(MAX_BODY));
        if status == 204 || status == 304 {
        } else if chunked {
            self.read_chunked(&mut body)?;
        } else if let Some(n) = length {
            self.take(n, &mut body)?;
        } else {
            body.extend_from_slice(&self.buf[self.pos..]);
            self.pos = self.buf.len();
            self.stream.as_mut().unwrap().read_to_end(&mut body)?;
            close = true;
        }
        if close {
            self.stream = None;
        }
        let wire_len = body.len();
        if gzip {
            body = inflate::gunzip(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        }
        Ok(Response { status, headers, body, wire_len, ttfb, total: t0.elapsed(), reused })
    }

    fn fill(&mut self) -> io::Result<()> {
        if self.pos == self.buf.len() {
            self.buf.clear();
            self.pos = 0;
        }
        let stream = self.stream.as_mut().unwrap();
        let old = self.buf.len();
        self.buf.resize(old + 32 * 1024, 0);
        loop {
            match stream.read(&mut self.buf[old..]) {
                Ok(0) => {
                    self.buf.truncate(old);
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed mid-response"));
                }
                Ok(n) => {
                    self.buf.truncate(old + n);
                    self.got_bytes = true;
                    quickack(stream.tcp());
                    return Ok(());
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    self.buf.truncate(old);
                    return Err(e);
                }
            }
        }
    }

    /// Moves exactly `n` body bytes into `out`: buffered bytes first, then straight from the socket.
    fn take(&mut self, n: usize, out: &mut Vec<u8>) -> io::Result<()> {
        if out.len() + n > MAX_BODY {
            return Err(invalid("response body too large"));
        }
        let have = (self.buf.len() - self.pos).min(n);
        out.extend_from_slice(&self.buf[self.pos..self.pos + have]);
        self.pos += have;
        if n > have {
            let start = out.len();
            out.resize(start + n - have, 0);
            self.stream.as_mut().unwrap().read_exact(&mut out[start..])?;
        }
        Ok(())
    }

    fn read_line(&mut self) -> io::Result<String> {
        loop {
            if let Some(i) = find(&self.buf[self.pos..], b"\r\n") {
                let line = String::from_utf8_lossy(&self.buf[self.pos..self.pos + i]).into_owned();
                self.pos += i + 2;
                return Ok(line);
            }
            if self.buf.len() - self.pos > MAX_HEAD {
                return Err(invalid("line too long"));
            }
            self.fill()?;
        }
    }

    fn read_chunked(&mut self, out: &mut Vec<u8>) -> io::Result<()> {
        loop {
            let line = self.read_line()?;
            let size = line.split(';').next().unwrap_or("").trim();
            let size = usize::from_str_radix(size, 16).map_err(|_| invalid("bad chunk size"))?;
            if size == 0 {
                while !self.read_line()?.is_empty() {}
                return Ok(());
            }
            self.take(size, out)?;
            if !self.read_line()?.is_empty() {
                return Err(invalid("missing CRLF after chunk"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn endpoint_parsing() {
        let e = Endpoint::parse("http://127.0.0.1/huntress-ctf-proxy", Some("secret")).unwrap();
        assert_eq!((e.addr.port(), e.host.as_str(), e.path.as_str()), (80, "127.0.0.1", "/huntress-ctf-proxy"));
        assert!(!e.has_auth(), "a token must never be sent over plain http");
        assert_eq!(e.target("op=status"), "/huntress-ctf-proxy?op=status");
        assert_eq!(e.target(""), "/huntress-ctf-proxy");
        let e = Endpoint::parse("http://127.0.0.1:8080", None).unwrap();
        assert_eq!(e.target("op=list"), "/?op=list");
        assert!(Endpoint::parse("ftp://x/", None).is_err());
    }

    /// A one-shot local server: replies to each request with the next canned response.
    fn serve(replies: Vec<Vec<u8>>) -> (std::thread::JoinHandle<Vec<String>>, String) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/p", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut seen = Vec::new();
            for r in replies {
                let mut buf = Vec::new();
                let mut byte = [0u8; 1];
                while !buf.ends_with(b"\r\n\r\n") {
                    s.read_exact(&mut byte).unwrap();
                    buf.push(byte[0]);
                }
                let head = String::from_utf8(buf).unwrap();
                if let Some(n) = head.lines().find_map(|l| l.strip_prefix("Content-Length: ")) {
                    let mut body = vec![0; n.trim().parse().unwrap()];
                    s.read_exact(&mut body).unwrap();
                }
                seen.push(head);
                s.write_all(&r).unwrap();
            }
            seen
        });
        (h, url)
    }

    #[test]
    fn keep_alive_chunked_and_gzip() {
        let gz: &[u8] = &[
            31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 203, 72, 205, 201, 201, 87, 200, 64, 39, 185, 0, 0, 136,
            89, 11, 24, 0, 0, 0,
        ];
        let mut r2 = b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
        r2.extend_from_slice(format!("{:x}\r\n", 10).as_bytes());
        r2.extend_from_slice(&gz[..10]);
        r2.extend_from_slice(format!("\r\n{:x}\r\n", gz.len() - 10).as_bytes());
        r2.extend_from_slice(&gz[10..]);
        r2.extend_from_slice(b"\r\n0\r\n\r\n");
        let (h, url) = serve(vec![b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi".to_vec(), r2]);
        let ep = Endpoint::parse(&url, None).unwrap();
        let mut c = Client::idle(&ep);
        let a = c.get("op=status").unwrap();
        assert_eq!((a.status, a.body.as_slice(), a.reused), (200, &b"hi"[..], false));
        let b = c.post_json("{\"x\":1}").unwrap();
        assert_eq!(b.body, b"hello hello hello hello\n");
        assert!(b.reused);
        assert_eq!(c.connects, 1);
        let seen = h.join().unwrap();
        assert!(seen[0].starts_with("GET /p?op=status HTTP/1.1\r\n"));
        assert!(seen[1].contains("Content-Type: application/json\r\nContent-Length: 7\r\n"));
        assert!(seen.iter().all(|h| h.contains("Accept-Encoding: gzip") && !h.contains("Authorization")));
    }
}
