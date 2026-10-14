//! OLE Compound File Binary (CFB) streams, and the MSI tables/summary information inside them.

use super::bytes::{cp1252, u16le, u32le, u64le, utf16le};
use std::collections::BTreeMap;

const END: u32 = 0xFFFF_FFFA;

/// MSI stream-name packing (`0x3800..0x4840` code points carry two/one base-64 symbols).
pub fn decode_stream_name(name: &str) -> String {
    const ALPHA: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz._";
    let mut out = String::new();
    for ch in name.chars() {
        let c = ch as u32;
        if (0x3800..0x4800).contains(&c) {
            let v = c - 0x3800;
            out.push(ALPHA[(v & 0x3F) as usize] as char);
            out.push(ALPHA[((v >> 6) & 0x3F) as usize] as char);
        } else if (0x4800..0x4840).contains(&c) {
            out.push(ALPHA[(c - 0x4800) as usize] as char);
        } else if c == 0x4840 {
            out.push('!');
        } else {
            out.push(ch);
        }
    }
    out
}

/// Every stream in the file, by decoded name.
pub fn streams(d: &[u8]) -> Option<BTreeMap<String, Vec<u8>>> {
    if !d.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return None;
    }
    let ssz = 1usize << u16le(d, 30)?;
    let mssz = 1usize << u16le(d, 32)?;
    let dirstart = u32le(d, 48)?;
    let cutoff = u32le(d, 56)? as u64;
    let minifat_start = u32le(d, 60)?;
    let nminifat = u32le(d, 64)?;
    let mut difat_sec = u32le(d, 68)?;
    let ndifat = u32le(d, 72)?;
    let sec = |i: u32| d.get(512 + i as usize * ssz..512 + (i as usize + 1) * ssz);
    let mut difat: Vec<u32> = (0..109).filter_map(|k| u32le(d, 76 + k * 4)).collect();
    for _ in 0..ndifat {
        let s = sec(difat_sec)?;
        let words: Vec<u32> = s.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        difat.extend_from_slice(&words[..words.len() - 1]);
        difat_sec = *words.last()?;
    }
    let mut fat: Vec<u32> = Vec::new();
    for &i in &difat {
        if i < END {
            fat.extend(sec(i)?.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())));
        }
    }
    let chain = |start: u32| -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = start;
        let mut seen = std::collections::HashSet::new();
        while i < END && seen.insert(i) {
            match sec(i) {
                Some(s) => out.extend_from_slice(s),
                None => break,
            }
            i = *fat.get(i as usize).unwrap_or(&END);
        }
        out
    };
    let dirs = chain(dirstart);
    let mut ents = Vec::new();
    for e in dirs.chunks_exact(128) {
        let nl = u16le(e, 64)? as usize;
        let name = utf16le(&e[..nl.saturating_sub(2).min(64)]);
        ents.push((name, e[66], u32le(e, 116)?, u64le(e, 120)?));
    }
    let root = ents.first()?;
    let mut ministream = chain(root.2);
    ministream.truncate(root.3 as usize);
    let minifat: Vec<u32> = if nminifat > 0 { chain(minifat_start).chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect() } else { Vec::new() };
    let mchain = |start: u32| -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = start;
        let mut seen = std::collections::HashSet::new();
        while i < END && seen.insert(i) {
            match ministream.get(i as usize * mssz..(i as usize + 1) * mssz) {
                Some(s) => out.extend_from_slice(s),
                None => break,
            }
            i = *minifat.get(i as usize).unwrap_or(&END);
        }
        out
    };
    let mut res = BTreeMap::new();
    for (name, typ, start, size) in &ents {
        if *typ != 2 {
            continue;
        }
        let mut raw = if *size >= cutoff { chain(*start) } else { mchain(*start) };
        raw.truncate(*size as usize);
        res.insert(decode_stream_name(name), raw);
    }
    Some(res)
}

/// The MSI string pool, plus the string-index width (2 or 3 bytes).
pub fn string_pool(st: &BTreeMap<String, Vec<u8>>) -> Option<(Vec<String>, usize)> {
    let pool = st.get("!_StringPool")?;
    let sd = st.get("!_StringData")?;
    let flags = u16le(pool, 2)?;
    let big = flags & 0x8000 != 0;
    let mut strs = vec![String::new()];
    let (mut pos, mut o) = (0usize, 4usize);
    while o + 4 <= pool.len() {
        let mut ln = u16le(pool, o)? as usize;
        let rc = u16le(pool, o + 2)? as usize;
        o += 4;
        if ln == 0 && rc != 0 {
            // Long string: the length continues in the next entry.
            ln = (rc << 16) | u16le(pool, o)? as usize;
            o += 4;
        }
        strs.push(cp1252(sd.get(pos..pos + ln).unwrap_or(&[])));
        pos += ln;
    }
    Some((strs, if big { 3 } else { 2 }))
}

/// The MSI `Property` table: name -> value.
pub fn msi_properties(d: &[u8]) -> Option<BTreeMap<String, String>> {
    let st = streams(d)?;
    let (strs, w) = string_pool(&st)?;
    let raw = st.get("!Property")?;
    let n = raw.len() / (2 * w);
    let rd = |off: usize| -> usize { raw[off..off + w].iter().rev().fold(0usize, |a, &b| (a << 8) | b as usize) };
    let mut out = BTreeMap::new();
    for i in 0..n {
        let k = strs.get(rd(i * w))?.clone();
        let v = strs.get(rd(n * w + i * w))?.clone();
        out.insert(k, v);
    }
    Some(out)
}

/// SummaryInformation property `pid` as a string (9 = revision number = package code).
pub fn summary_string(d: &[u8], pid: u32) -> Option<String> {
    let st = streams(d)?;
    let si = st.iter().find(|(k, _)| k.ends_with("SummaryInformation") && !k.contains("Document")).map(|(_, v)| v)?;
    let off = u32le(si, 44)? as usize;
    let cnt = u32le(si, off + 4)? as usize;
    for i in 0..cnt {
        let id = u32le(si, off + 8 + i * 8)?;
        let po = u32le(si, off + 12 + i * 8)? as usize;
        if id == pid {
            let ln = u32le(si, off + po + 4)? as usize;
            let raw = si.get(off + po + 8..off + po + 8 + ln)?;
            let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
            return Some(cp1252(&raw[..end]));
        }
    }
    None
}
