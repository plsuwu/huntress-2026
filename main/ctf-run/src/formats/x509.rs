//! X.509 certificates: located as PEM blocks or raw DER anywhere in a file (binaries, packages, mail and
//! captures embed them), parsed just far enough for fingerprints and the usual identity fields.

use crate::codec;
use crate::formats::bytes::find;

#[derive(Debug, Clone)]
pub struct Cert {
    /// Byte offset of the PEM block or DER sequence in the file.
    pub offset: usize,
    pub pem: bool,
    pub der: Vec<u8>,
    pub version: u8,
    /// Serial number as big-endian hex (leading zero bytes kept as encoded).
    pub serial: String,
    pub sig_alg: String,
    pub issuer: Vec<(String, String)>,
    pub subject: Vec<(String, String)>,
    pub not_before: String,
    pub not_after: String,
    pub key_alg: String,
    pub san: Vec<String>,
}

/// One DER TLV: (tag, content, total length).
fn tlv(b: &[u8]) -> Option<(u8, &[u8], usize)> {
    let tag = *b.first()?;
    let l0 = *b.get(1)? as usize;
    let (len, hdr) = if l0 < 0x80 {
        (l0, 2)
    } else {
        let n = l0 & 0x7f;
        if n == 0 || n > 4 {
            return None;
        }
        let mut v = 0usize;
        for i in 0..n {
            v = v << 8 | *b.get(2 + i)? as usize;
        }
        (v, 2 + n)
    };
    let body = b.get(hdr..hdr.checked_add(len)?)?;
    Some((tag, body, hdr + len))
}

/// The TLVs of a constructed value's content.
fn children(mut b: &[u8]) -> Option<Vec<(u8, &[u8])>> {
    let mut out = Vec::new();
    while !b.is_empty() {
        let (t, c, n) = tlv(b)?;
        out.push((t, c));
        b = &b[n..];
    }
    Some(out)
}

pub fn oid(b: &[u8]) -> String {
    let Some(&first) = b.first() else { return String::new() };
    let mut parts = vec![(first / 40).min(2) as u64, (first as u64).saturating_sub(40 * (first / 40).min(2) as u64)];
    let mut v = 0u64;
    for &x in &b[1..] {
        v = v << 7 | (x & 0x7f) as u64;
        if x & 0x80 == 0 {
            parts.push(v);
            v = 0;
        }
    }
    parts.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(".")
}

fn oid_name(o: &str) -> String {
    match o {
        "2.5.4.3" => "CN",
        "2.5.4.6" => "C",
        "2.5.4.7" => "L",
        "2.5.4.8" => "ST",
        "2.5.4.10" => "O",
        "2.5.4.11" => "OU",
        "2.5.4.5" => "serialNumber",
        "1.2.840.113549.1.9.1" => "emailAddress",
        "0.9.2342.19200300.100.1.25" => "DC",
        "1.3.101.112" => "ed25519",
        "1.3.101.113" => "ed448",
        "1.2.840.113549.1.1.1" => "rsaEncryption",
        "1.2.840.113549.1.1.11" => "sha256WithRSAEncryption",
        "1.2.840.113549.1.1.12" => "sha384WithRSAEncryption",
        "1.2.840.113549.1.1.13" => "sha512WithRSAEncryption",
        "1.2.840.113549.1.1.5" => "sha1WithRSAEncryption",
        "1.2.840.10045.2.1" => "ecPublicKey",
        "1.2.840.10045.4.3.2" => "ecdsa-with-SHA256",
        "1.2.840.10045.4.3.3" => "ecdsa-with-SHA384",
        _ => return o.to_string(),
    }
    .to_string()
}

fn string(tag: u8, c: &[u8]) -> String {
    if tag == 0x1e {
        let u: Vec<u16> = c.chunks_exact(2).map(|p| u16::from_be_bytes([p[0], p[1]])).collect();
        return String::from_utf16_lossy(&u);
    }
    String::from_utf8_lossy(c).into_owned()
}

fn name(c: &[u8]) -> Option<Vec<(String, String)>> {
    let mut out = Vec::new();
    for (_, set) in children(c)? {
        for (_, atv) in children(set)? {
            let kv = children(atv)?;
            let (_, o) = kv.first()?;
            let (t, v) = kv.get(1)?;
            out.push((oid_name(&oid(o)), string(*t, v)));
        }
    }
    Some(out)
}

/// UTCTime / GeneralizedTime -> ISO 8601 `YYYY-MM-DDTHH:MM:SSZ`.
fn time(tag: u8, c: &[u8]) -> String {
    let s = String::from_utf8_lossy(c);
    let s = s.trim_end_matches('Z');
    let full = match tag {
        0x17 if s.len() >= 10 => {
            let yy: u32 = s[..2].parse().unwrap_or(0);
            format!("{}{}", if yy >= 50 { 19 } else { 20 }, s)
        }
        _ => s.to_string(),
    };
    if full.len() < 12 {
        return full;
    }
    let sec = full.get(12..14).unwrap_or("00");
    format!("{}-{}-{}T{}:{}:{}Z", &full[..4], &full[4..6], &full[6..8], &full[8..10], &full[10..12], sec)
}

impl Cert {
    pub fn parse_der(der: &[u8], offset: usize, pem: bool) -> Option<Cert> {
        let (t, body, n) = tlv(der)?;
        if t != 0x30 {
            return None;
        }
        let der = &der[..n];
        let top = children(body)?;
        if top.len() != 3 || top[0].0 != 0x30 || top[1].0 != 0x30 || top[2].0 != 0x03 {
            return None;
        }
        let tbs = children(top[0].1)?;
        let mut i = 0;
        let mut version = 1;
        if tbs.first()?.0 == 0xa0 {
            version = children(tbs[0].1)?.first().and_then(|(_, v)| v.last()).map(|v| v + 1).unwrap_or(1);
            i = 1;
        }
        let (st, serial) = *tbs.get(i)?;
        if st != 0x02 {
            return None;
        }
        let sig_alg = children(tbs.get(i + 1)?.1)?.first().map(|(_, o)| oid_name(&oid(o))).unwrap_or_default();
        let issuer = name(tbs.get(i + 2)?.1)?;
        let validity = children(tbs.get(i + 3)?.1)?;
        let subject = name(tbs.get(i + 4)?.1)?;
        let key_alg = children(tbs.get(i + 5)?.1)?.first().and_then(|(_, a)| children(a)).and_then(|a| a.first().map(|(_, o)| oid_name(&oid(o)))).unwrap_or_default();
        let mut san = Vec::new();
        for (t, ext) in &tbs[i + 6..] {
            if *t != 0xa3 {
                continue;
            }
            for (_, e) in children(children(ext)?.first()?.1)? {
                let f = children(e)?;
                if oid(f.first()?.1) != "2.5.29.17" {
                    continue;
                }
                let (_, os) = f.last()?;
                for (gt, g) in children(children(os)?.first()?.1)? {
                    match gt {
                        0x82 | 0x81 | 0x86 => san.push(String::from_utf8_lossy(g).into_owned()),
                        0x87 if g.len() == 4 => san.push(format!("{}.{}.{}.{}", g[0], g[1], g[2], g[3])),
                        _ => {}
                    }
                }
            }
        }
        Some(Cert {
            offset,
            pem,
            der: der.to_vec(),
            version,
            serial: crate::formats::bytes::hex(serial),
            sig_alg,
            issuer,
            subject,
            not_before: validity.first().map(|(t, c)| time(*t, c)).unwrap_or_default(),
            not_after: validity.get(1).map(|(t, c)| time(*t, c)).unwrap_or_default(),
            key_alg,
            san,
        })
    }

    pub fn attr<'a>(n: &'a [(String, String)], k: &str) -> Option<&'a str> {
        n.iter().find(|(a, _)| a.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str())
    }

    /// RFC 4514-ish one-line name, most significant RDN first as encoded (`C=US, O=..., CN=...`).
    pub fn dn(n: &[(String, String)]) -> String {
        n.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ")
    }

    pub fn self_issued(&self) -> bool {
        self.issuer == self.subject
    }
}

/// Every certificate in a file: PEM blocks first (in order), then raw DER sequences not inside a PEM block.
pub fn find_all(b: &[u8]) -> Vec<Cert> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = find(&b[at..], b"-----BEGIN ").map(|i| i + at) {
        let Some(e) = find(&b[i + 11..], b"-----").map(|e| e + i + 11) else { break };
        let label = String::from_utf8_lossy(&b[i + 11..e]).into_owned();
        let body_start = e + 5;
        let end_marker = format!("-----END {label}-----");
        let Some(j) = find(&b[body_start..], end_marker.as_bytes()).map(|j| j + body_start) else { break };
        if label.contains("CERTIFICATE") && !label.contains("REQUEST") {
            let text = String::from_utf8_lossy(&b[body_start..j]);
            // Skip RFC 1421 headers (`Key: value` lines) if any.
            let body: String = text.lines().filter(|l| !l.contains(':')).collect::<Vec<_>>().join("");
            if let Some(der) = codec::base64(&body) {
                if let Some(c) = Cert::parse_der(&der, i, true) {
                    out.push(c);
                }
            }
        }
        at = j + end_marker.len();
    }
    // Raw DER: SEQUENCE(long form) directly holding a SEQUENCE(long form) tbsCertificate.
    let mut k = 0;
    while k + 8 < b.len() {
        if b[k] == 0x30 && (b[k + 1] == 0x82 || b[k + 1] == 0x81) {
            let inner = if b[k + 1] == 0x82 { k + 4 } else { k + 3 };
            if b.get(inner) == Some(&0x30) {
                if let Some(c) = Cert::parse_der(&b[k..], k, false) {
                    let n = c.der.len();
                    out.push(c);
                    k += n;
                    continue;
                }
            }
        }
        k += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEM: &str = "-----BEGIN CERTIFICATE-----
MIIB3zCCAZGgAwIBAgIJAKtUqYzrHwrSMAUGAytlcDBOMQswCQYDVQQGEwJVUzEV
MBMGA1UEChMMSHVudHJlc3MgQ1RGMSgwJgYDVQQDEx9hcmVuYS1kZWY0MTRkNmIz
OWIuZXhhbXBsZS50ZXN0MB4XDTI2MTAwMTAwMDAwMFoXDTI3MTAwMTAwMDAwMFow
TjELMAkGA1UEBhMCVVMxFTATBgNVBAoTDEh1bnRyZXNzIENURjEoMCYGA1UEAxMf
YXJlbmEtZGVmNDE0ZDZiMzliLmV4YW1wbGUudGVzdDAqMAUGAytlcAMhAAOhB7/z
zhC+HXDdGOdLwJln5NYwm6UNXx3chmQSVTG4o4GLMIGIMA4GA1UdDwEB/wQEAwIH
gDAdBgNVHSUEFjAUBggrBgEFBQcDAQYIKwYBBQUHAwIwDAYDVR0TAQH/BAIwADAd
BgNVHQ4EFgQUVkdap1RjR0wChd9dvyvKtz2mUTUwKgYDVR0RBCMwIYIfYXJlbmEt
ZGVmNDE0ZDZiMzliLmV4YW1wbGUudGVzdDAFBgMrZXADQQBqvlC6+UX7K/pxitEm
Ryk/pjGiP6bpKyTR0IS6chuMYv59kCjnRraxQWBqX3gsbSdmqM1nCgGzSem7gPa9
0wgB
-----END CERTIFICATE-----
";

    #[test]
    fn pem_and_der() {
        let c = find_all(PEM.as_bytes());
        assert_eq!(c.len(), 1);
        let c = &c[0];
        assert_eq!(c.version, 3);
        assert_eq!(c.serial, "00ab54a98ceb1f0ad2");
        assert_eq!(Cert::attr(&c.subject, "CN"), Some("arena-def414d6b39b.example.test"));
        assert_eq!(c.not_before, "2026-10-01T00:00:00Z");
        assert_eq!(c.key_alg, "ed25519");
        assert_eq!(c.san, vec!["arena-def414d6b39b.example.test"]);
        let mut blob = b"junk\x30\x82".to_vec();
        blob.extend_from_slice(&c.der);
        let d = find_all(&blob);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].der, c.der);
    }
}
