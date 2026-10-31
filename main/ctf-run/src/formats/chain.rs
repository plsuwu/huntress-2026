//! Artifact graphs (d31): a case zip of `artifacts/<id>` files (no extensions; PowerShell text, PNG, SQLite,
//! email, zip) where each artifact names the next one in its primary metadata: a script's launch command
//! (`Start-Process '.\<id>'`, `Invoke-Item`, `&`), a PNG `tEXt` value (`next=<id>`, `done ...` = end), a SQLite
//! text cell equal to an id (`launch_queue.target`), an email attachment body (`forwarded to: <id>`, `open: <id>`),
//! a zip member (`run.txt`: `execute <id>`). Decoys mention ids only in secondary places (subjects, notes, comments,
//! readmes) or form a cycle with no entry point.

use crate::formats::cluster::Cluster;
use crate::formats::{email, media, sqlite, zip::Zip};

pub struct Node {
    pub id: String,
    pub kind: &'static str,
    /// Ids this artifact points at (primary metadata only), in order of appearance.
    pub next: Vec<String>,
    /// Explicit end marker (PNG `done`), or no pointer at all.
    pub terminal: bool,
    /// A pointer whose syntax is broken (mismatched script quotes).
    pub broken: bool,
    pub evidence: String,
}

pub struct Graph {
    pub nodes: Vec<Node>,
}

fn is_tok(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// Whole-token occurrences of known ids in `text`, in order.
fn ids_in(text: &str, ids: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in text.char_indices() {
        if i > 0 && text[..i].chars().next_back().is_some_and(is_tok) {
            continue;
        }
        let mut best: Option<&String> = None;
        for id in ids {
            if text[i..].starts_with(id.as_str()) && !text[i + id.len()..].chars().next().is_some_and(is_tok) && best.is_none_or(|b| b.len() < id.len()) {
                best = Some(id);
            }
        }
        if let Some(b) = best {
            out.push(b.clone());
        }
    }
    out
}

fn text_of(b: &[u8]) -> Option<&str> {
    let s = std::str::from_utf8(b).ok()?;
    (!s.chars().any(|c| c.is_control() && !c.is_whitespace())).then_some(s)
}

/// Script launch lines: ids outside `#` comments; `broken` when the quotes around the id do not match.
fn script(s: &str, ids: &[String], n: &mut Node) {
    for line in s.lines() {
        let code = line.split('#').next().unwrap_or("");
        for id in ids_in(code, ids) {
            let at = code.find(&id).unwrap_or(0);
            let before = code[..at].trim_end_matches(['.', '\\', '/']).chars().next_back();
            let after = code[at + id.len()..].chars().next();
            let q = |c: Option<char>| c.filter(|c| *c == '\'' || *c == '"');
            if q(before) != q(after) {
                n.broken = true;
            }
            n.evidence.push_str(&format!("[{}] ", line.trim()));
            n.next.push(id);
        }
    }
}

impl Graph {
    pub fn parse(b: &[u8]) -> Option<Graph> {
        let c = Cluster::parse(b)?;
        let ids: Vec<String> = c.artifacts.iter().map(|a| a.id.clone()).collect();
        let mut nodes = Vec::new();
        for a in &c.artifacts {
            let d = a.bytes.as_deref().unwrap_or(&[]);
            let mut n = Node { id: a.id.clone(), kind: "unknown", next: Vec::new(), terminal: false, broken: false, evidence: String::new() };
            if let Some((chunks, _)) = media::png_chunks(d) {
                n.kind = "png";
                for ch in chunks.iter().filter(|ch| ch.kind == "tEXt" || ch.kind == "iTXt") {
                    let body = &d[ch.offset + 8..ch.offset + 8 + ch.len];
                    let v = String::from_utf8_lossy(body.split(|x| *x == 0).last().unwrap_or(&[])).into_owned();
                    let found = ids_in(&v, &ids);
                    if found.is_empty() && v.trim_start().to_ascii_lowercase().starts_with("done") {
                        n.terminal = true;
                    }
                    n.evidence.push_str(&format!("[{} {}] ", ch.kind, v.trim()));
                    n.next.extend(found);
                }
            } else if let Some(db) = sqlite::Db::parse(d) {
                n.kind = "sqlite";
                for t in db.tables() {
                    for (_, row) in db.rows(t.rootpage as u32) {
                        for v in row.iter().filter_map(sqlite::Val::text) {
                            let v = v.trim().trim_start_matches(".\\").trim_start_matches("./");
                            if ids.iter().any(|id| id == v) {
                                n.evidence.push_str(&format!("[{} {}] ", t.name, v));
                                n.next.push(v.to_string());
                            }
                        }
                    }
                }
            } else if d.starts_with(b"PK\x03\x04") {
                n.kind = "zip";
                if let Ok(z) = Zip::parse(d) {
                    for e in z.files() {
                        let Ok(m) = z.read(e, b"infected") else { continue };
                        let Some(s) = text_of(&m) else { continue };
                        let found = ids_in(s, &ids);
                        if !found.is_empty() {
                            n.evidence.push_str(&format!("[{}: {}] ", e.name, s.trim()));
                        }
                        n.next.extend(found);
                    }
                }
            } else if let Some(m) = email::Message::parse(d).filter(|m| m.header("From").is_some() || m.header("Message-ID").is_some()) {
                n.kind = "email";
                for p in m.attachments() {
                    let Some(s) = text_of(&p.data) else { continue };
                    let found = ids_in(s, &ids);
                    n.evidence.push_str(&format!("[{:?}: {}] ", p.filename(), s.trim()));
                    n.next.extend(found);
                }
            } else if let Some(s) = text_of(d) {
                n.kind = "script";
                script(s, &ids, &mut n);
            }
            n.next.retain(|x| *x != n.id);
            n.next.dedup();
            if n.next.is_empty() {
                n.terminal = true;
            }
            nodes.push(n);
        }
        Some(Graph { nodes })
    }

    fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Nodes that point somewhere but that nothing points at.
    pub fn roots(&self) -> Vec<&str> {
        self.nodes.iter().filter(|n| !n.next.is_empty() && !self.nodes.iter().any(|m| m.next.contains(&n.id))).map(|n| n.id.as_str()).collect()
    }

    /// The walk from `root` along single pointers; `Err` on a fork. `Ok((path, closed))`, closed = ended at a
    /// terminal node (not in a cycle).
    pub fn walk(&self, root: &str) -> Result<(Vec<String>, bool), String> {
        let mut path = vec![root.to_string()];
        let mut cur = self.node(root).ok_or("no root")?;
        loop {
            match cur.next.as_slice() {
                [] => return Ok((path, true)),
                [x] => {
                    if path.contains(x) {
                        return Ok((path, false));
                    }
                    path.push(x.clone());
                    cur = self.node(x).ok_or("dangling")?;
                }
                many => return Err(format!("fork at {} -> {many:?}", cur.id)),
            }
        }
    }

    /// The one acyclic root-to-terminal chain.
    pub fn execution_chain(&self) -> Result<Vec<String>, String> {
        let mut found = Vec::new();
        for r in self.roots() {
            if let Ok((p, true)) = self.walk(r) {
                if p.len() > 1 {
                    found.push(p);
                }
            }
        }
        match found.len() {
            1 => Ok(found.pop().unwrap()),
            n => Err(format!("{n} candidate chains {found:?}")),
        }
    }

    /// Ids on cycles (no entry point): the tempting loop.
    pub fn cycle_ids(&self) -> Vec<String> {
        let mut out = Vec::new();
        for n in &self.nodes {
            let mut cur = n;
            let mut seen = vec![n.id.as_str()];
            while let [x] = cur.next.as_slice() {
                if x == &n.id {
                    out.push(n.id.clone());
                    break;
                }
                if seen.contains(&x.as_str()) {
                    break;
                }
                seen.push(x);
                match self.node(x) {
                    Some(m) => cur = m,
                    None => break,
                }
            }
        }
        out
    }

    pub fn facts(&self) -> String {
        let f: Vec<String> = self
            .nodes
            .iter()
            .map(|n| format!("{} {} -> {:?}{}{} {}", n.id, n.kind, n.next, if n.terminal { " END" } else { "" }, if n.broken { " BROKEN" } else { "" }, n.evidence.trim()))
            .collect();
        format!("graph {} node(s), roots {:?}: {}", self.nodes.len(), self.roots(), f.join(" | "))
    }
}
