//! Android packages: the binary XML (AXML) manifest, `resources.arsc` string lookups for `@0x7f..`
//! references, and the signer certificates (JAR-signature PKCS#7 blocks and the v2/v3 APK Signing Block).
//! Attribute names resolve through the resource map first (`android:` attributes are identified by id,
//! so a renamed string-pool entry cannot hide them), then the string pool.

use super::bytes::{u16le, u32le};
use super::zip::Zip;

#[derive(Clone, Debug)]
pub struct Attr {
    pub ns: String,
    pub name: String,
    pub res_id: Option<u32>,
    /// Typed value rendered the way aapt/apktool print it (strings verbatim, ints decimal, bools
    /// `true`/`false`, references `@0x7f..`).
    pub value: String,
    pub data_type: u8,
    pub data: u32,
}

#[derive(Clone, Debug)]
pub struct Element {
    pub depth: usize,
    pub name: String,
    pub attrs: Vec<Attr>,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&Attr> {
        self.attrs.iter().find(|a| a.name == name)
    }
}

/// `android:` attribute names by framework resource id (public.xml).
fn framework_attr(id: u32) -> Option<&'static str> {
    Some(match id {
        0x0101_0000 => "theme",
        0x0101_0001 => "label",
        0x0101_0002 => "icon",
        0x0101_0003 => "name",
        0x0101_0006 => "permission",
        0x0101_000b => "process",
        0x0101_000c => "taskAffinity",
        0x0101_000f => "debuggable",
        0x0101_0010 => "exported",
        0x0101_0011 => "enabled",
        0x0101_001d => "launchMode",
        0x0101_0020 => "description",
        0x0101_0024 => "value",
        0x0101_0025 => "resource",
        0x0101_0026 => "mimeType",
        0x0101_0027 => "scheme",
        0x0101_0028 => "host",
        0x0101_020c => "minSdkVersion",
        0x0101_021b => "versionCode",
        0x0101_021c => "versionName",
        0x0101_0270 => "targetSdkVersion",
        0x0101_0271 => "maxSdkVersion",
        0x0101_0280 => "allowBackup",
        0x0101_02b7 => "installLocation",
        0x0101_0572 => "compileSdkVersion",
        0x0101_0573 => "compileSdkVersionCodename",
        0x0101_04ec => "usesCleartextTraffic",
        0x0101_0285 => "allowTaskReparenting",
        _ => return None,
    })
}

/// A ResStringPool chunk at `at`: its strings.
fn string_pool(b: &[u8], at: usize) -> Option<Vec<String>> {
    let count = u32le(b, at + 8)? as usize;
    let flags = u32le(b, at + 16)?;
    let start = u32le(b, at + 20)? as usize;
    let hsize = u16le(b, at + 2)? as usize;
    let utf8 = flags & 0x100 != 0;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let off = at + start + u32le(b, at + hsize + i * 4)? as usize;
        out.push(if utf8 {
            let len8 = |p: usize| -> Option<(usize, usize)> {
                let x = *b.get(p)? as usize;
                Some(if x & 0x80 != 0 { (((x & 0x7f) << 8) | *b.get(p + 1)? as usize, 2) } else { (x, 1) })
            };
            let (_, n1) = len8(off)?;
            let (n, n2) = len8(off + n1)?;
            String::from_utf8_lossy(b.get(off + n1 + n2..off + n1 + n2 + n)?).into_owned()
        } else {
            let x = u16le(b, off)? as usize;
            let (n, skip) = if x & 0x8000 != 0 { (((x & 0x7fff) << 16) | u16le(b, off + 2)? as usize, 4) } else { (x, 2) };
            let units: Vec<u16> = (0..n).map(|k| u16le(b, off + skip + k * 2)).collect::<Option<_>>()?;
            String::from_utf16_lossy(&units)
        });
    }
    Some(out)
}

fn render(data_type: u8, data: u32, strings: &[String], raw: Option<&String>) -> String {
    match data_type {
        0x03 => strings.get(data as usize).or(raw).cloned().unwrap_or_default(),
        0x01 => format!("@0x{data:08x}"),
        0x02 => format!("?0x{data:08x}"),
        0x10 => (data as i32).to_string(),
        0x11 => format!("0x{data:08x}"),
        0x12 => (data != 0).to_string(),
        0x04 => f32::from_bits(data).to_string(),
        _ => raw.cloned().unwrap_or_else(|| format!("0x{data:08x}")),
    }
}

/// Parses a binary XML document into its elements (document order, with nesting depth).
pub fn axml(b: &[u8]) -> Option<Vec<Element>> {
    if u16le(b, 0)? != 0x0003 {
        return None;
    }
    let end = (u32le(b, 4)? as usize).min(b.len());
    let mut p = u16le(b, 2)? as usize;
    let (mut strings, mut ids): (Vec<String>, Vec<u32>) = (Vec::new(), Vec::new());
    let mut out = Vec::new();
    let mut depth = 0usize;
    let s = |v: &Vec<String>, i: u32| if i == u32::MAX { String::new() } else { v.get(i as usize).cloned().unwrap_or_default() };
    while p + 8 <= end {
        let (kind, hsize, size) = (u16le(b, p)?, u16le(b, p + 2)? as usize, u32le(b, p + 4)? as usize);
        if size < 8 {
            break;
        }
        match kind {
            0x0001 => strings = string_pool(b, p)?,
            0x0180 => ids = (0..(size - hsize) / 4).filter_map(|i| u32le(b, p + hsize + i * 4)).collect(),
            0x0102 => {
                let x = p + hsize;
                let name = s(&strings, u32le(b, x + 4)?);
                let (astart, asize, acount) = (u16le(b, x + 8)? as usize, u16le(b, x + 10)? as usize, u16le(b, x + 12)? as usize);
                let mut attrs = Vec::new();
                for i in 0..acount {
                    let a = x + astart + i * asize.max(20);
                    let ni = u32le(b, a + 4)?;
                    let res_id = ids.get(ni as usize).copied().filter(|&r| r != 0);
                    let ns = s(&strings, u32le(b, a)?);
                    let pooled = s(&strings, ni);
                    let name = res_id.and_then(framework_attr).map(str::to_string).unwrap_or(pooled);
                    let raw_i = u32le(b, a + 8)?;
                    let raw = (raw_i != u32::MAX).then(|| s(&strings, raw_i));
                    let (data_type, data) = (*b.get(a + 15)?, u32le(b, a + 16)?);
                    attrs.push(Attr { ns, name, res_id, value: render(data_type, data, &strings, raw.as_ref()), data_type, data });
                }
                out.push(Element { depth, name, attrs });
                depth += 1;
            }
            0x0103 => depth = depth.saturating_sub(1),
            _ => {}
        }
        p += size;
    }
    Some(out)
}

/// A plain-text manifest (some "APKs" carry source XML): elements and their attributes, `android:`
/// prefixes stripped.
pub fn text_xml(b: &[u8]) -> Option<Vec<Element>> {
    let t = std::str::from_utf8(b).ok()?;
    if !t.trim_start().starts_with('<') {
        return None;
    }
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut rest = t;
    while let Some(i) = rest.find('<') {
        rest = &rest[i + 1..];
        let end = rest.find('>')?;
        let tag = &rest[..end];
        rest = &rest[end + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        if tag.starts_with('/') {
            depth = depth.saturating_sub(1);
            continue;
        }
        let self_close = tag.ends_with('/');
        let tag = tag.trim_end_matches('/');
        let name_end = tag.find(char::is_whitespace).unwrap_or(tag.len());
        let mut attrs = Vec::new();
        let mut a = &tag[name_end..];
        while let Some(eq) = a.find('=') {
            let key = a[..eq].trim();
            let after = a[eq + 1..].trim_start();
            let q = after.chars().next()?;
            let close = after[1..].find(q)? + 1;
            let val = &after[1..close];
            let (ns, name) = key.rsplit_once(':').map(|(n, k)| (n.to_string(), k.to_string())).unwrap_or((String::new(), key.to_string()));
            let value = val.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
            attrs.push(Attr { ns, name, res_id: None, value, data_type: 3, data: 0 });
            a = &after[close + 1..];
        }
        out.push(Element { depth, name: tag[..name_end].to_string(), attrs });
        if !self_close {
            depth += 1;
        }
    }
    Some(out)
}

/// `resources.arsc`: the value of resource `id` (strings and inline values; first matching config,
/// the default config preferred).
pub fn arsc_lookup(b: &[u8], id: u32) -> Option<String> {
    if u16le(b, 0)? != 0x0002 {
        return None;
    }
    let mut p = u16le(b, 2)? as usize;
    let mut global = Vec::new();
    let (want_pkg, want_type, want_entry) = (id >> 24, (id >> 16) & 0xff, (id & 0xffff) as usize);
    let mut found: Vec<(bool, String)> = Vec::new();
    while p + 8 <= b.len() {
        let (kind, size) = (u16le(b, p)?, u32le(b, p + 4)? as usize);
        if size < 8 {
            break;
        }
        match kind {
            0x0001 => global = string_pool(b, p)?,
            0x0200 if u32le(b, p + 8)? == want_pkg => {
                let mut q = p + u16le(b, p + 2)? as usize;
                while q + 8 <= p + size {
                    let (k, hs, sz) = (u16le(b, q)?, u16le(b, q + 2)? as usize, u32le(b, q + 4)? as usize);
                    if sz < 8 {
                        break;
                    }
                    if k == 0x0201 && *b.get(q + 8)? as u32 == want_type {
                        let count = u32le(b, q + 12)? as usize;
                        let start = u32le(b, q + 16)? as usize;
                        let cfg_size = u32le(b, q + 20)? as usize;
                        let default = b.get(q + 24..q + 20 + cfg_size).map(|c| c.iter().all(|&x| x == 0)).unwrap_or(false);
                        if want_entry < count {
                            let off = u32le(b, q + hs + want_entry * 4)?;
                            if off != u32::MAX {
                                let e = q + start + off as usize;
                                let flags = u16le(b, e + 2)?;
                                if flags & 1 == 0 {
                                    let v = e + u16le(b, e)? as usize;
                                    let (dt, data) = (*b.get(v + 3)?, u32le(b, v + 4)?);
                                    found.push((default, render(dt, data, &global, None)));
                                }
                            }
                        }
                    }
                    q += sz;
                }
            }
            _ => {}
        }
        p += size;
    }
    found.iter().find(|f| f.0).or(found.first()).map(|f| f.1.clone())
}

/// A parsed package.
pub struct Apk {
    pub manifest: Vec<Element>,
    pub binary_manifest: bool,
    pub arsc: Option<Vec<u8>>,
    /// Signer certificates (DER), JAR signature files first, then the v2/v3 signing block.
    pub certs: Vec<(String, Vec<u8>)>,
    pub members: Vec<String>,
}

impl Apk {
    pub fn parse(b: &[u8]) -> Option<Apk> {
        let z = Zip::parse(b).ok()?;
        let m = z.read_name("AndroidManifest.xml", b"").ok()?;
        let (manifest, binary_manifest) = match axml(&m) {
            Some(e) => (e, true),
            None => (text_xml(&m)?, false),
        };
        let arsc = z.read_name("resources.arsc", b"").ok();
        let mut certs = Vec::new();
        for e in z.files() {
            let up = e.name.to_ascii_uppercase();
            if up.starts_with("META-INF/") && (up.ends_with(".RSA") || up.ends_with(".DSA") || up.ends_with(".EC")) {
                if let Ok(d) = z.read(e, b"") {
                    for c in pkcs7_certs(&d) {
                        certs.push((e.name.clone(), c));
                    }
                }
            }
        }
        certs.extend(signing_block_certs(b));
        Some(Apk { manifest, binary_manifest, arsc, certs, members: z.files().map(|e| e.name.clone()).collect() })
    }

    pub fn element(&self, name: &str) -> Option<&Element> {
        self.manifest.iter().find(|e| e.name == name)
    }

    /// A manifest attribute with references resolved through `resources.arsc` when possible.
    pub fn value(&self, element: &str, attr: &str) -> Option<String> {
        let a = self.element(element)?.attr(attr)?;
        if a.data_type == 0x01 {
            if let Some(v) = self.arsc.as_deref().and_then(|r| arsc_lookup(r, a.data)) {
                return Some(v);
            }
        }
        Some(a.value.clone())
    }

    /// The launcher activity: first activity (or alias) whose intent filter has MAIN + LAUNCHER.
    pub fn launcher_activity(&self) -> Option<String> {
        let m = &self.manifest;
        for (i, e) in m.iter().enumerate() {
            if e.name != "activity" && e.name != "activity-alias" {
                continue;
            }
            let kids: Vec<&Element> = m[i + 1..].iter().take_while(|k| k.depth > e.depth).collect();
            let has = |el: &str, v: &str| kids.iter().any(|k| k.name == el && k.attr("name").map(|a| a.value == v).unwrap_or(false));
            if has("action", "android.intent.action.MAIN") && has("category", "android.intent.category.LAUNCHER") {
                let n = e.attr("name")?.value.clone();
                let pkg = self.value("manifest", "package").unwrap_or_default();
                return Some(if n.starts_with('.') { format!("{pkg}{n}") } else if !n.contains('.') { format!("{pkg}.{n}") } else { n });
            }
        }
        None
    }
}

/// DER TLV at `p`: (tag, header length, content length).
fn tlv(b: &[u8], p: usize) -> Option<(u8, usize, usize)> {
    let tag = *b.get(p)?;
    let l = *b.get(p + 1)? as usize;
    if l < 0x80 {
        return Some((tag, 2, l));
    }
    let n = l & 0x7f;
    if n == 0 || n > 4 {
        return None;
    }
    let mut len = 0usize;
    for i in 0..n {
        len = (len << 8) | *b.get(p + 2 + i)? as usize;
    }
    Some((tag, 2 + n, len))
}

/// Children of the constructed DER value whose content spans `[s, e)`.
fn children(b: &[u8], mut s: usize, e: usize) -> Vec<(u8, usize, usize)> {
    let mut v = Vec::new();
    while s < e {
        let Some((t, h, l)) = tlv(b, s) else { break };
        if s + h + l > e {
            break;
        }
        v.push((t, s, h + l));
        s += h + l;
    }
    v
}

/// Certificates (DER) carried in a PKCS#7 SignedData blob.
pub fn pkcs7_certs(b: &[u8]) -> Vec<Vec<u8>> {
    let go = || -> Option<Vec<Vec<u8>>> {
        let (_, h, l) = tlv(b, 0)?;
        let ci = children(b, h, h + l);
        let &(_, p0, _) = ci.iter().find(|c| c.0 == 0xa0)?;
        let (_, h0, _) = tlv(b, p0)?;
        let (_, hs, ls) = tlv(b, p0 + h0)?;
        let sd = children(b, p0 + h0 + hs, p0 + h0 + hs + ls);
        let &(_, pc, _) = sd.iter().find(|c| c.0 == 0xa0)?;
        let (_, hc, lc) = tlv(b, pc)?;
        Some(children(b, pc + hc, pc + hc + lc).into_iter().filter(|c| c.0 == 0x30).map(|(_, s, n)| b[s..s + n].to_vec()).collect())
    };
    go().unwrap_or_default()
}

/// Certificates in the APK Signing Block (v2 `0x7109871a`, v3 `0xf05368c0`, v3.1 `0x1b93ad61`), found
/// just before the central directory.
pub fn signing_block_certs(b: &[u8]) -> Vec<(String, Vec<u8>)> {
    let go = || -> Option<Vec<(String, Vec<u8>)>> {
        let eocd = super::zip::find_eocd(b)?;
        let cd = u32le(b, eocd + 16)? as usize;
        if b.get(cd.checked_sub(16)?..cd)? != b"APK Sig Block 42" {
            return None;
        }
        let size = super::bytes::u64le(b, cd - 24)? as usize;
        let start = (cd + 8).checked_sub(size + 8)?;
        let mut p = start + 8;
        let mut out = Vec::new();
        let lp = |b: &[u8], p: usize| -> Option<(usize, usize)> { Some((p + 4, u32le(b, p)? as usize)) };
        while p + 12 <= cd - 24 {
            let len = super::bytes::u64le(b, p)? as usize;
            let id = u32le(b, p + 8)?;
            let (vs, ve) = (p + 12, p + 8 + len);
            let scheme = match id {
                0x7109_871a => "v2",
                0xf053_68c0 => "v3",
                0x1b93_ad61 => "v3.1",
                _ => "",
            };
            if !scheme.is_empty() {
                // value: signers (u32 len) -> signer (u32 len) -> signed data (u32 len) -> digests, certificates
                let (s0, n0) = lp(b, vs)?;
                let mut q = s0;
                while q < (s0 + n0).min(ve) {
                    let (signer, slen) = lp(b, q)?;
                    let (sd, _) = lp(b, signer)?;
                    let (dg, dlen) = lp(b, sd)?;
                    let (cs, clen) = lp(b, dg + dlen)?;
                    let mut c = cs;
                    while c < cs + clen {
                        let (cert, n) = lp(b, c)?;
                        out.push((format!("APK Signing Block {scheme}"), b.get(cert..cert + n)?.to_vec()));
                        c = cert + n;
                    }
                    q = signer + slen;
                }
            }
            p = ve;
        }
        Some(out)
    };
    go().unwrap_or_default()
}

/// The X.509 subject/issuer as `CN=..., O=..., C=..` (RFC 4514 order: most specific first) plus the
/// serial number and validity, from a DER certificate.
pub struct CertInfo {
    pub subject: Vec<(String, String)>,
    pub issuer: Vec<(String, String)>,
    pub serial_hex: String,
    pub not_before: String,
    pub not_after: String,
}

fn oid_name(o: &[u8]) -> String {
    match o {
        [0x55, 0x04, 0x03] => "CN".into(),
        [0x55, 0x04, 0x06] => "C".into(),
        [0x55, 0x04, 0x07] => "L".into(),
        [0x55, 0x04, 0x08] => "ST".into(),
        [0x55, 0x04, 0x0a] => "O".into(),
        [0x55, 0x04, 0x0b] => "OU".into(),
        [0x55, 0x04, 0x05] => "SERIALNUMBER".into(),
        _ => super::bytes::hex(o),
    }
}

fn name_rdns(b: &[u8], p: usize) -> Option<Vec<(String, String)>> {
    let (_, h, l) = tlv(b, p)?;
    let mut out = Vec::new();
    for (_, s, _) in children(b, p + h, p + h + l) {
        let (_, hs, ls) = tlv(b, s)?;
        for (_, a, _) in children(b, s + hs, s + hs + ls) {
            let (_, ha, la) = tlv(b, a)?;
            let kv = children(b, a + ha, a + ha + la);
            let (o, v) = (kv.first()?, kv.get(1)?);
            let (_, ho, lo) = tlv(b, o.1)?;
            let (vt, hv, lv) = tlv(b, v.1)?;
            let raw = b.get(v.1 + hv..v.1 + hv + lv)?;
            let val = if vt == 0x1e { super::bytes::utf16le(&raw.chunks(2).flat_map(|c| [*c.get(1).unwrap_or(&0), c[0]]).collect::<Vec<_>>()) } else { String::from_utf8_lossy(raw).into_owned() };
            out.push((oid_name(&b[o.1 + ho..o.1 + ho + lo]), val));
        }
    }
    Some(out)
}

pub fn cert_info(c: &[u8]) -> Option<CertInfo> {
    let (_, h, l) = tlv(c, 0)?;
    let top = children(c, h, h + l);
    let (_, tbs, _) = *top.first()?;
    let (_, ht, lt) = tlv(c, tbs)?;
    let mut f = children(c, tbs + ht, tbs + ht + lt);
    if f.first()?.0 == 0xa0 {
        f.remove(0);
    }
    let (_, sp, _) = *f.first()?;
    let (_, hs, ls) = tlv(c, sp)?;
    let serial = c.get(sp + hs..sp + hs + ls)?;
    let serial = if serial.len() > 1 && serial[0] == 0 { &serial[1..] } else { serial };
    let issuer = name_rdns(c, f.get(2)?.1)?;
    let (_, vp, _) = *f.get(3)?;
    let (_, hv, lv) = tlv(c, vp)?;
    let times: Vec<String> = children(c, vp + hv, vp + hv + lv)
        .into_iter()
        .filter_map(|(_, s, _)| {
            let (_, h, l) = tlv(c, s)?;
            Some(String::from_utf8_lossy(c.get(s + h..s + h + l)?).into_owned())
        })
        .collect();
    let subject = name_rdns(c, f.get(4)?.1)?;
    Some(CertInfo { subject, issuer, serial_hex: super::bytes::hex(serial), not_before: times.first()?.clone(), not_after: times.get(1)?.clone() })
}

/// RFC 4514-style string: RDNs in reverse encoding order, `, `-joined.
pub fn dn_string(rdns: &[(String, String)]) -> String {
    rdns.iter().rev().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ")
}
