//! Office Open XML packages (docx/xlsx/pptx): relationship parts and core properties, read with a
//! minimal tag/attribute scanner (no DTDs, no namespaces beyond stripping the prefix).

use super::zip::Zip;

#[derive(Debug, Clone)]
pub struct Rel {
    /// The `.rels` part this relationship came from.
    pub part: String,
    pub id: String,
    pub kind: String,
    pub target: String,
    pub external: bool,
}

pub struct Package {
    /// `(part name, bytes)` in central-directory order.
    pub parts: Vec<(String, Vec<u8>)>,
}

impl Package {
    pub fn parse(data: &[u8]) -> Option<Package> {
        let z = Zip::parse(data).ok()?;
        let parts: Vec<(String, Vec<u8>)> = z.files().filter_map(|e| Some((e.name.clone(), z.read(e, b"").ok()?))).collect();
        parts.iter().any(|(n, _)| n == "[Content_Types].xml").then_some(Package { parts })
    }

    fn text(&self, name: &str) -> Option<String> {
        self.parts.iter().find(|(n, _)| n == name).map(|(_, b)| String::from_utf8_lossy(b).trim_start_matches('\u{feff}').to_string())
    }

    /// Every `<Relationship>` of every `.rels` part, parts in directory order, relationships in
    /// document order.
    pub fn relationships(&self) -> Vec<Rel> {
        let mut out = Vec::new();
        for (name, b) in &self.parts {
            if !name.ends_with(".rels") {
                continue;
            }
            let xml = String::from_utf8_lossy(b);
            for (tag, attrs) in tags(&xml) {
                if local(&tag) != "Relationship" {
                    continue;
                }
                let get = |k: &str| attrs.iter().find(|(n, _)| local(n) == k).map(|(_, v)| v.clone()).unwrap_or_default();
                out.push(Rel {
                    part: name.clone(),
                    id: get("Id"),
                    kind: get("Type"),
                    target: get("Target"),
                    external: get("TargetMode").eq_ignore_ascii_case("external"),
                });
            }
        }
        out
    }

    /// `docProps/core.xml` (or wherever the package relationship points) as `(local name, text)`.
    pub fn core_properties(&self) -> Vec<(String, String)> {
        let path = self
            .relationships()
            .into_iter()
            .find(|r| r.part == "_rels/.rels" && r.kind.ends_with("/core-properties"))
            .map(|r| r.target.trim_start_matches('/').to_string())
            .unwrap_or_else(|| "docProps/core.xml".into());
        self.text(&path).map(|x| leaf_elements(&x)).unwrap_or_default()
    }

    pub fn app_properties(&self) -> Vec<(String, String)> {
        self.text("docProps/app.xml").map(|x| leaf_elements(&x)).unwrap_or_default()
    }
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Single-pass XML entity decoding (`&amp;lt;` stays `&lt;`).
pub fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(j) = tail.find(';').filter(|&j| j <= 12) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let ent = &tail[1..j];
        let c = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &tail[j + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Start/empty tags with their (unescaped) attributes, in document order.
pub fn tags(xml: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut out = Vec::new();
    let b = xml.as_bytes();
    let mut i = 0;
    while let Some(off) = xml[i..].find('<') {
        let s = i + off + 1;
        if s >= b.len() || matches!(b[s], b'/' | b'?' | b'!') {
            i = s;
            continue;
        }
        let mut p = s;
        while p < b.len() && !b[p].is_ascii_whitespace() && b[p] != b'>' && b[p] != b'/' {
            p += 1;
        }
        let name = xml[s..p].to_string();
        let mut attrs = Vec::new();
        loop {
            while p < b.len() && b[p].is_ascii_whitespace() {
                p += 1;
            }
            if p >= b.len() || b[p] == b'>' || b[p] == b'/' {
                break;
            }
            let ns = p;
            while p < b.len() && b[p] != b'=' && !b[p].is_ascii_whitespace() && b[p] != b'>' {
                p += 1;
            }
            let an = xml[ns..p].to_string();
            while p < b.len() && (b[p].is_ascii_whitespace() || b[p] == b'=') {
                p += 1;
            }
            if p >= b.len() || !(b[p] == b'"' || b[p] == b'\'') {
                break;
            }
            let q = b[p];
            let vs = p + 1;
            let Some(ve) = b[vs..].iter().position(|&c| c == q).map(|x| x + vs) else { break };
            attrs.push((an, unescape(&xml[vs..ve])));
            p = ve + 1;
        }
        out.push((name, attrs));
        i = p.max(s);
    }
    out
}

/// `<x>text</x>` elements without children, as (local name, unescaped text).
fn leaf_elements(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = xml[i..].find('<') {
        let s = i + off + 1;
        i = s;
        if xml[s..].starts_with(['/', '?', '!']) {
            continue;
        }
        let Some(gt) = xml[s..].find('>').map(|g| g + s) else { break };
        if xml[..gt].ends_with('/') {
            continue;
        }
        let name = xml[s..gt].split_ascii_whitespace().next().unwrap_or("").to_string();
        let close = format!("</{name}>");
        let body_end = match xml[gt + 1..].find('<') {
            Some(o) => gt + 1 + o,
            None => break,
        };
        if xml[body_end..].starts_with(&close) {
            out.push((local(&name).to_string(), unescape(&xml[gt + 1..body_end])));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rels_and_core() {
        let rels = r#"<?xml version="1.0"?><Relationships xmlns="x"><Relationship Id="rId9" Type="t/hyperlink" Target="https://e.test/a?x=1&amp;y=2" TargetMode="External"/><Relationship Id='rId2' Type="t/styles" Target="styles.xml"/></Relationships>"#;
        let t = tags(rels);
        assert_eq!(t[1].0, "Relationship");
        assert_eq!(t[1].1[2], ("Target".to_string(), "https://e.test/a?x=1&y=2".to_string()));
        assert_eq!(t[2].1[0].1, "rId2");
        let core = r#"<cp:coreProperties><dc:title>A &amp;lt; B</dc:title><dcterms:created xsi:type="W3CDTF">2026-10-01T09:00:00Z</dcterms:created><cp:keywords/></cp:coreProperties>"#;
        let l = leaf_elements(core);
        assert_eq!(l, [("title".to_string(), "A &lt; B".to_string()), ("created".to_string(), "2026-10-01T09:00:00Z".to_string())]);
    }
}
