//! Nested-archive registry: a delivery and every zip inside it (recursively), each mapped to its
//! members by base name - the Rust counterpart of `utils/solver_py_scripts/nested_zip.py`.
//!
//! Question labels address files as `[delivery N: ]<zip name> -> <member>`. The delivery itself is
//! registered under its own file name and as [`OUTER`], so labels naming the download resolve too.
//! Eager: suitable for small evidence (metadata, script bundles). Large binaries should stream
//! members with `formats::zip::Zip` instead.

use crate::formats::zip::{self, Zip};

pub const OUTER: &str = "__outer__.zip";
pub const PASSWORD: &[u8] = b"infected";

pub struct Registry {
    /// (zip name, [(member base name, bytes)]) in discovery order.
    pub zips: Vec<(String, Vec<(String, Vec<u8>)>)>,
}

/// `delivery 1: a.zip -> b.jpg` -> ("a.zip", "b.jpg"); member "" when absent.
pub fn parse_label(label: &str) -> (&str, &str) {
    let (z, m) = match label.split_once("->") {
        Some((z, m)) => (z.trim(), m.trim()),
        None => (label.trim(), ""),
    };
    (z.rsplit(": ").next().unwrap_or(z).trim(), m)
}

impl Registry {
    /// Registers `data` (a zip, or a single non-zip file) under `name` and [`OUTER`].
    pub fn load(name: &str, data: &[u8]) -> Result<Registry, String> {
        let mut r = Registry { zips: Vec::new() };
        if zip::is_zip(data) {
            r.add(OUTER, data, 0)?;
            let outer = r.zips[0].1.clone();
            r.zips.push((name.to_string(), outer));
        } else {
            r.zips.push((OUTER.to_string(), vec![(name.to_string(), data.to_vec())]));
        }
        Ok(r)
    }

    fn add(&mut self, name: &str, data: &[u8], depth: u8) -> Result<(), String> {
        let z = Zip::parse(data).map_err(|e| format!("{name}: {e}"))?;
        let mut members = Vec::new();
        let mut nested = Vec::new();
        for e in z.files() {
            let b = z.read(e, PASSWORD).map_err(|err| format!("{name}: {err}"))?;
            let base = e.base_name().to_string();
            if base.to_ascii_lowercase().ends_with(".zip") && zip::is_zip(&b) && depth < 4 {
                nested.push((base.clone(), b.clone()));
            }
            members.push((base, b));
        }
        self.zips.push((name.to_string(), members));
        for (n, b) in nested {
            // An inner zip that fails to parse stays available as plain member bytes.
            let _ = self.add(&n, &b, depth + 1);
        }
        Ok(())
    }

    pub fn zip(&self, name: &str) -> Option<&[(String, Vec<u8>)]> {
        self.zips.iter().find(|(n, _)| n == name).map(|(_, m)| m.as_slice())
    }

    /// (members of the named zip, member name, member bytes). Falls back to the outer archive
    /// when the named zip is not registered (single-solve downloads are the named zip itself).
    pub fn resolve<'a>(&'a self, label: &'a str) -> (&'a [(String, Vec<u8>)], &'a str, Option<&'a [u8]>) {
        let (z, m) = parse_label(label);
        let members = self.zip(z).or_else(|| self.zip(OUTER)).unwrap_or(&[]);
        let data = if m.is_empty() { None } else { members.iter().find(|(n, _)| n == m).map(|(_, b)| b.as_slice()) };
        (members, m, data)
    }

    pub fn has_zip(&self, name: &str) -> bool {
        self.zip(name).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(parse_label("delivery 1: a.zip -> b.jpg"), ("a.zip", "b.jpg"));
        assert_eq!(parse_label("a.zip"), ("a.zip", ""));
        let data = std::fs::read("testdata/plain.zip").unwrap();
        let r = Registry::load("evidence.zip", &data).unwrap();
        let (_, m, d) = r.resolve("evidence.zip -> a.txt");
        assert_eq!((m, d.map(|d| d.len())), ("a.txt", Some(500)));
        let (_, _, d) = r.resolve("other.zip -> b.bin");
        assert_eq!(d.map(|d| d.len()), Some(256));
    }
}
