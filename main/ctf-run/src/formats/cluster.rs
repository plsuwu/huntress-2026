//! Artifact clusters (d30): a zip of `artifacts/*` plus a `delivery-manifest.json`
//! (`huntress.artifact_delivery_manifest.v1`: id, filename, path, family, producer_id, size_bytes, sha256 per
//! artifact). Relations are computed from the real bytes; the manifest only names artifacts (by `id`) and
//! carries the claims (sha256, producer) that the bytes can contradict.

use crate::crypto::sha256::sha256;
use crate::formats::bytes::hex;
use crate::formats::inflate::unzlib;
use crate::formats::media;
use crate::formats::zip::{self, Zip};
use crate::json::Value;

pub const MANIFEST: &str = "delivery-manifest.json";

pub struct Artifact {
    pub id: String,
    pub path: String,
    pub family: String,
    pub producer: String,
    /// The manifest's claimed sha256 / size (empty / None when the manifest has no entry).
    pub claimed_sha256: String,
    pub claimed_size: Option<u64>,
    pub bytes: Option<Vec<u8>>,
    pub sha256: String,
}

pub struct Cluster {
    /// Manifest order, then files the manifest does not list (id = file stem).
    pub artifacts: Vec<Artifact>,
    pub manifest: bool,
}

fn stem(path: &str) -> &str {
    let base = path.rsplit('/').next().unwrap_or(path);
    base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base)
}

impl Cluster {
    pub fn parse(b: &[u8]) -> Option<Cluster> {
        let z = Zip::parse(b).ok()?;
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for e in z.files() {
            files.push((e.name.clone(), z.read(e, b"infected").ok()?));
        }
        let man = files.iter().find(|(n, _)| n.rsplit('/').next() == Some(MANIFEST)).and_then(|(_, m)| Value::parse_bytes(m).ok());
        let mut artifacts = Vec::new();
        let mut used = vec![false; files.len()];
        if let Some(list) = man.as_ref().and_then(|m| m.get("artifacts")).and_then(Value::as_array) {
            for a in list {
                let s = |k: &str| a.str_of(k).unwrap_or("").to_string();
                let path = if a.str_of("path").is_some() { s("path") } else { s("filename") };
                let i = files.iter().position(|(n, _)| *n == path).or_else(|| files.iter().position(|(n, _)| n.rsplit('/').next() == Some(&s("filename"))));
                let bytes = i.map(|i| {
                    used[i] = true;
                    files[i].1.clone()
                });
                let id = if a.str_of("id").is_some() { s("id") } else { stem(&path).to_string() };
                artifacts.push(Artifact {
                    sha256: bytes.as_deref().map(|b| hex(&sha256(b))).unwrap_or_default(),
                    id,
                    path,
                    family: s("family"),
                    producer: s("producer_id"),
                    claimed_sha256: s("sha256").to_ascii_lowercase(),
                    claimed_size: a.get("size_bytes").and_then(Value::as_i64).map(|v| v as u64),
                    bytes,
                });
            }
        }
        for (i, (n, d)) in files.iter().enumerate() {
            if used[i] || n.ends_with('/') || n.rsplit('/').next() == Some(MANIFEST) {
                continue;
            }
            artifacts.push(Artifact {
                id: stem(n).to_string(),
                path: n.clone(),
                family: String::new(),
                producer: String::new(),
                claimed_sha256: String::new(),
                claimed_size: None,
                sha256: hex(&sha256(d)),
                bytes: Some(d.clone()),
            });
        }
        (!artifacts.is_empty()).then_some(Cluster { artifacts, manifest: man.is_some() })
    }

    /// Ids grouped by `key` (artifacts without bytes/key are skipped); only groups of 2+, ids sorted, groups
    /// ordered by their first id. d30 graded this shape for `identical-sha256-groups`.
    pub fn groups(&self, key: impl Fn(&Artifact) -> Option<Vec<u8>>) -> Vec<Vec<String>> {
        let mut g: Vec<(Vec<u8>, Vec<String>)> = Vec::new();
        for a in &self.artifacts {
            let Some(k) = key(a) else { continue };
            match g.iter_mut().find(|(gk, _)| *gk == k) {
                Some((_, ids)) => ids.push(a.id.clone()),
                None => g.push((k, vec![a.id.clone()])),
            }
        }
        let mut out: Vec<Vec<String>> = g.into_iter().map(|(_, mut ids)| {
            ids.sort();
            ids
        }).filter(|ids| ids.len() > 1).collect();
        out.sort();
        out
    }

    pub fn identical_groups(&self) -> Vec<Vec<String>> {
        self.groups(|a| a.bytes.as_ref().map(|_| a.sha256.clone().into_bytes()))
    }

    pub fn semantic_groups(&self) -> Vec<Vec<String>> {
        self.groups(|a| a.bytes.as_deref().map(semantic_key))
    }

    /// Artifacts whose bytes contradict the manifest's sha256 / size claim (or are missing).
    pub fn claim_mismatches(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .artifacts
            .iter()
            .filter(|a| self.manifest && (a.bytes.is_none() || (!a.claimed_sha256.is_empty() && a.claimed_sha256 != a.sha256) || a.claimed_size.is_some_and(|s| Some(s) != a.bytes.as_ref().map(|b| b.len() as u64))))
            .map(|a| a.id.clone())
            .collect();
        v.sort();
        v
    }

    /// (container id, contained id): an artifact zip with a member byte-identical to another artifact.
    pub fn containment(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for a in &self.artifacts {
            let Some(b) = a.bytes.as_deref().filter(|b| zip::is_zip(b)) else { continue };
            let Ok(z) = Zip::parse(b) else { continue };
            for e in z.files() {
                let Ok(m) = z.read(e, b"infected") else { continue };
                let h = hex(&sha256(&m));
                for o in &self.artifacts {
                    if o.id != a.id && o.bytes.is_some() && o.sha256 == h {
                        out.push((a.id.clone(), o.id.clone()));
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// A content key that ignores encoding noise: text (BOM, CRLF, trailing whitespace), JSON (key order,
/// whitespace), PNG (IHDR + decompressed IDAT), zip (sorted member names + content hashes).
pub fn semantic_key(b: &[u8]) -> Vec<u8> {
    if let Some((chunks, _)) = media::png_chunks(b) {
        let mut idat = Vec::new();
        let mut ihdr = Vec::new();
        for c in &chunks {
            let Some(body) = b.get(c.offset + 8..c.offset + 8 + c.len) else { continue };
            match c.kind.as_str() {
                "IHDR" => ihdr.extend_from_slice(body),
                "IDAT" => idat.extend_from_slice(body),
                _ => {}
            }
        }
        if let Ok(px) = unzlib(&idat) {
            let mut k = b"png:".to_vec();
            k.extend_from_slice(&ihdr);
            k.extend_from_slice(&sha256(&px));
            return k;
        }
    }
    if zip::is_zip(b) {
        if let Ok(z) = Zip::parse(b) {
            let mut m: Vec<String> = z.files().map(|e| format!("{}={}", e.name, z.read(e, b"infected").map(|d| hex(&sha256(&semantic_key(&d)))).unwrap_or_default())).collect();
            m.sort();
            return format!("zip:{}", m.join("\n")).into_bytes();
        }
    }
    let t = b.strip_prefix(b"\xef\xbb\xbf").unwrap_or(b);
    if let Ok(s) = std::str::from_utf8(t) {
        if let Ok(v) = Value::parse(s) {
            if matches!(v, Value::Obj(_) | Value::Arr(_)) {
                return format!("json:{}", canonical(&v)).into_bytes();
            }
        }
        let lines: Vec<&str> = s.split('\n').map(|l| l.trim_end()).collect();
        return format!("text:{}", lines.join("\n").trim_end()).into_bytes();
    }
    b.to_vec()
}

fn canonical(v: &Value) -> String {
    match v {
        Value::Obj(m) => {
            let mut kv: Vec<(String, String)> = m.iter().map(|(k, v)| (k.clone(), canonical(v))).collect();
            kv.sort();
            let mut s = String::from("{");
            for (i, (k, v)) in kv.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                crate::json::write_str(&mut s, k);
                s.push(':');
                s.push_str(v);
            }
            s.push('}');
            s
        }
        Value::Arr(a) => format!("[{}]", a.iter().map(canonical).collect::<Vec<_>>().join(",")),
        _ => v.to_json(),
    }
}
