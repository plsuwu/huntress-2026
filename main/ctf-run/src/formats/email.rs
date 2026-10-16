//! RFC 5322 / MIME messages (`.eml`): unfolded headers, RFC 2047 encoded words, and the leaf parts
//! of a multipart body with their transfer encoding removed.

use crate::codec;

pub struct Part {
    pub headers: Vec<(String, String)>,
    /// Body with Content-Transfer-Encoding (base64 / quoted-printable) removed.
    pub data: Vec<u8>,
}

impl Part {
    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }

    /// `filename=` of Content-Disposition, else `name=` of Content-Type.
    pub fn filename(&self) -> Option<String> {
        self.header("Content-Disposition")
            .and_then(|v| param(v, "filename"))
            .or_else(|| self.header("Content-Type").and_then(|v| param(v, "name")))
            .map(|s| decode_words(&s))
    }

    pub fn is_attachment(&self) -> bool {
        self.header("Content-Disposition").is_some_and(|v| v.trim().to_ascii_lowercase().starts_with("attachment")) || self.filename().is_some()
    }

    /// Lowercased media type without parameters (`text/plain` when absent).
    pub fn content_type(&self) -> String {
        self.header("Content-Type").map(|v| v.split(';').next().unwrap_or("").trim().to_ascii_lowercase()).unwrap_or_else(|| "text/plain".into())
    }
}

pub struct Message {
    /// (name as written, unfolded raw value with surrounding whitespace trimmed), in file order.
    pub headers: Vec<(String, String)>,
    /// Raw body bytes after the blank line.
    pub body: Vec<u8>,
}

fn header<'a>(hs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    hs.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

/// Splits a header block from its body; `None` unless the first line is a header.
fn split_headers(b: &[u8]) -> Option<(Vec<(String, String)>, Vec<u8>)> {
    let text = String::from_utf8_lossy(b);
    let text = text.trim_start_matches('\u{feff}');
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut pos = 0;
    let mut body_at = text.len();
    for line in text.split_inclusive('\n') {
        let start = pos;
        pos += line.len();
        let l = line.trim_end_matches(['\r', '\n']);
        if l.is_empty() {
            body_at = pos;
            break;
        }
        if l.starts_with([' ', '\t']) {
            let (_, v) = headers.last_mut()?;
            // Unfolding removes the CRLF only; the folding whitespace stays.
            v.push_str(l);
            continue;
        }
        let (n, v) = l.split_once(':')?;
        if n.is_empty() || n.bytes().any(|c| c <= b' ' || c >= 0x7f) {
            if headers.is_empty() {
                return None;
            }
            body_at = start;
            break;
        }
        headers.push((n.to_string(), v.to_string()));
    }
    for (_, v) in &mut headers {
        *v = v.trim().to_string();
    }
    (!headers.is_empty()).then(|| (headers, text.as_bytes()[body_at.min(text.len())..].to_vec()))
}

impl Message {
    pub fn parse(b: &[u8]) -> Option<Message> {
        let (headers, body) = split_headers(b)?;
        let known = ["from", "to", "subject", "date", "message-id", "received", "mime-version", "return-path"];
        headers.iter().any(|(n, _)| known.contains(&n.to_ascii_lowercase().as_str())).then_some(Message { headers, body })
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }

    pub fn all(&self, name: &str) -> Vec<&str> {
        self.headers.iter().filter(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()).collect()
    }

    /// Leaf MIME parts in document order (the message itself when not multipart).
    pub fn parts(&self) -> Vec<Part> {
        let mut out = Vec::new();
        leaves(&self.headers, &self.body, &mut out, 0);
        out
    }

    pub fn attachments(&self) -> Vec<Part> {
        self.parts().into_iter().filter(Part::is_attachment).collect()
    }
}

fn leaves(hs: &[(String, String)], body: &[u8], out: &mut Vec<Part>, depth: u8) {
    let ct = header(hs, "Content-Type").unwrap_or("");
    if ct.trim().to_ascii_lowercase().starts_with("multipart/") && depth < 8 {
        if let Some(bd) = param(ct, "boundary") {
            for chunk in multipart_chunks(body, &bd) {
                match split_headers(&chunk) {
                    Some((h, b)) => leaves(&h, &b, out, depth + 1),
                    None => leaves(&[], strip_leading_newline(&chunk), out, depth + 1),
                }
            }
            return;
        }
    }
    let enc = header(hs, "Content-Transfer-Encoding").unwrap_or("").trim().to_ascii_lowercase();
    let data = match enc.as_str() {
        "base64" => {
            let s: String = String::from_utf8_lossy(body).chars().filter(|c| !c.is_whitespace()).collect();
            codec::base64(&s).unwrap_or_else(|| body.to_vec())
        }
        "quoted-printable" => quoted_printable(body),
        _ => body.to_vec(),
    };
    out.push(Part { headers: hs.to_vec(), data });
}

fn strip_leading_newline(b: &[u8]) -> &[u8] {
    b.strip_prefix(b"\r\n").or_else(|| b.strip_prefix(b"\n")).unwrap_or(b)
}

/// Bodies between `--boundary` lines (the CRLF before each delimiter belongs to the delimiter).
fn multipart_chunks(body: &[u8], boundary: &str) -> Vec<Vec<u8>> {
    let delim = format!("--{boundary}");
    let mut out = Vec::new();
    let mut cur: Option<Vec<u8>> = None;
    for line in body.split_inclusive(|&c| c == b'\n') {
        let l = String::from_utf8_lossy(line);
        let t = l.trim_end();
        if t == delim || t == format!("{delim}--") {
            if let Some(mut c) = cur.take() {
                if c.ends_with(b"\n") {
                    c.pop();
                    if c.ends_with(b"\r") {
                        c.pop();
                    }
                }
                out.push(c);
            }
            if t.ends_with("--") && t != delim {
                break;
            }
            cur = Some(Vec::new());
            continue;
        }
        if let Some(c) = cur.as_mut() {
            c.extend_from_slice(line);
        }
    }
    out
}

/// `key=value` / `key="value"` parameter of a structured header (case-insensitive key; RFC 2231
/// `key*=charset''pct` accepted).
pub fn param(v: &str, key: &str) -> Option<String> {
    for seg in split_params(v).into_iter().skip(1) {
        let Some((k, val)) = seg.split_once('=') else { continue };
        let k = k.trim().to_ascii_lowercase();
        let val = val.trim();
        if k == key {
            return Some(unquote(val));
        }
        if k == format!("{key}*") {
            let raw = unquote(val);
            let enc = raw.splitn(3, '\'').nth(2).unwrap_or(&raw).to_string();
            return Some(codec::percent_decode_all(&enc));
        }
    }
    None
}

fn split_params(v: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut q = false;
    let mut esc = false;
    for c in v.chars() {
        if esc {
            out.last_mut().unwrap().push(c);
            esc = false;
            continue;
        }
        match c {
            '\\' if q => {
                out.last_mut().unwrap().push(c);
                esc = true;
            }
            '"' => {
                q = !q;
                out.last_mut().unwrap().push(c);
            }
            ';' if !q => out.push(String::new()),
            _ => out.last_mut().unwrap().push(c),
        }
    }
    out
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        let mut out = String::new();
        let mut it = s[1..s.len() - 1].chars();
        while let Some(c) = it.next() {
            if c == '\\' {
                if let Some(n) = it.next() {
                    out.push(n);
                }
            } else {
                out.push(c);
            }
        }
        out
    } else {
        s.to_string()
    }
}

pub fn quoted_printable(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' {
            if b.get(i + 1) == Some(&b'\r') && b.get(i + 2) == Some(&b'\n') {
                i += 3;
                continue;
            }
            if b.get(i + 1) == Some(&b'\n') {
                i += 2;
                continue;
            }
            if let Some(h) = b.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()) {
                out.push(h);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// RFC 2047 `=?charset?B|Q?text?=` words decoded (UTF-8 / ASCII / Latin-1); whitespace between
/// adjacent encoded words is dropped.
pub fn decode_words(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    let mut pending_ws = String::new();
    let mut last_encoded = false;
    while !rest.is_empty() {
        if let Some((txt, used)) = rest.strip_prefix("=?").and_then(encoded_word) {
            if !last_encoded {
                out.push_str(&pending_ws);
            }
            pending_ws.clear();
            out.push_str(&txt);
            rest = &rest[2 + used..];
            last_encoded = true;
            continue;
        }
        let c = rest.chars().next().unwrap();
        if c.is_whitespace() {
            pending_ws.push(c);
        } else {
            out.push_str(&pending_ws);
            pending_ws.clear();
            out.push(c);
            last_encoded = false;
        }
        rest = &rest[c.len_utf8()..];
    }
    out.push_str(&pending_ws);
    out
}

/// `charset?E?text?=` (after the `=?`) -> (decoded text, bytes consumed).
fn encoded_word(s: &str) -> Option<(String, usize)> {
    let mut it = s.splitn(3, '?');
    let cs = it.next()?;
    let enc = it.next()?;
    let tail = it.next()?;
    let end = tail.find("?=")?;
    let text = &tail[..end];
    if text.contains(char::is_whitespace) {
        return None;
    }
    let bytes = match enc {
        "B" | "b" => codec::base64(text)?,
        "Q" | "q" => quoted_printable(text.replace('_', " ").as_bytes()),
        _ => return None,
    };
    let cs = cs.split('*').next().unwrap_or(cs).to_ascii_lowercase();
    let s2 = if cs.contains("8859-1") || cs.contains("latin1") || cs.contains("1252") {
        bytes.iter().map(|&b| b as char).collect()
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    Some((s2, cs.len() + enc.len() + end + 4))
}

/// `<a@b> <c@d>` -> ["<a@b>", "<c@d>"] (msg-id tokens of Message-ID / In-Reply-To / References).
pub fn msg_ids(v: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = v;
    while let Some(i) = rest.find('<') {
        let Some(j) = rest[i..].find('>') else { break };
        out.push(rest[i..i + j + 1].chars().filter(|c| !c.is_whitespace()).collect());
        rest = &rest[i + j + 1..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multipart() {
        let m = Message::parse(b"From: a <a@x>\r\nSubject: =?utf-8?B?SGk=?= =?utf-8?Q?_there?=\r\nMessage-ID:\r\n <id@x>\r\nContent-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nhello\r\n--b\r\nContent-Disposition: attachment; filename=\"n.txt\"\r\nContent-Transfer-Encoding: base64\r\n\r\naGk=\r\n--b--\r\n").unwrap();
        assert_eq!(m.header("message-id"), Some("<id@x>"));
        assert_eq!(decode_words(m.header("subject").unwrap()), "Hi there");
        let p = m.parts();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].data, b"hello");
        assert_eq!(p[1].filename().as_deref(), Some("n.txt"));
        assert_eq!(p[1].data, b"hi");
        assert_eq!(msg_ids("<a@b>\r\n <c@d>"), vec!["<a@b>", "<c@d>"]);
    }
}
