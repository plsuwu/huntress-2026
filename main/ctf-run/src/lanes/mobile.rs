//! Mobile Android: every APK carries `assets/huntress_puzzle.json`
//! (`{"schema":"huntress.mobile.puzzle.v1","codec":..,"data":..,"key"?,"parts"?,"order"?}`) whose decode is
//! the answer `ANS-` + 16 uppercase hex. The Android story (intents, providers, WebViews, TLS) is decoration.
//!
//! Known codecs are decoded directly; an unknown one goes through a bounded search over byte
//! transforms (reverse, hex, base64, base32, rot13, xor/sub with any key, seeded PRNG keystreams).
//! Only a single `ANS-...` result is accepted.
//!
//! Since d23 (`schema` v2, `codec:"analysis"`, no `data`) the code is computed by the app itself:
//! `Release.code(Context)` in `classes.dex` builds `ANS-` + hex of a long run through mixers, probes,
//! JNI natives (`lib/x86_64/*.so`) and secondary DEX files loaded by reflection. `dex_code` runs every
//! static `String` method of the primary DEX in `formats::dalvik` (with `formats::x64emu` for natives).

use super::{Analysed, Analysis, Lane};
use crate::codec;
use crate::formats::dex::Dex;
use crate::formats::zip::{self, Zip};
use crate::formats::dalvik;
use crate::formats::elf::Elf;
use crate::formats::x64emu;
use crate::json::Value;
use crate::model::{Delivery, Question, RunCtx};

pub struct Mobile;

const PUZZLE: &str = "assets/huntress_puzzle.json";

/// The `ANS-XXXXXXXXXXXXXXXX` codes appearing in `b`.
fn ans_codes(b: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 20 <= b.len() {
        if &b[i..i + 4] == b"ANS-" && b[i + 4..i + 20].iter().all(|c| c.is_ascii_digit() || (b'A'..=b'F').contains(c)) {
            let next_hex = b.get(i + 20).is_some_and(|c| c.is_ascii_hexdigit());
            if !next_hex {
                out.push(String::from_utf8_lossy(&b[i..i + 20]).into_owned());
            }
            i += 20;
        } else {
            i += 1;
        }
    }
    out
}

fn is_answer(b: &[u8]) -> Option<String> {
    let t = std::str::from_utf8(b).ok()?.trim();
    (t.len() == 20 && ans_codes(t.as_bytes()).len() == 1).then(|| t.to_string())
}

fn keys(p: &Value) -> Vec<i64> {
    let mut k = Vec::new();
    for name in ["key", "shift", "k"] {
        match p.get(name) {
            Some(v @ Value::Num(_)) => k.extend(v.as_i64()),
            Some(Value::Arr(a)) => k.extend(a.iter().filter_map(Value::as_i64)),
            _ => {}
        }
    }
    if let Some(a) = p.get("keys").and_then(Value::as_array) {
        k.extend(a.iter().filter_map(Value::as_i64));
    }
    k
}

/// The integer `order` array, if any.
fn order(p: &Value) -> Option<Vec<usize>> {
    p.get("order")?.as_array()?.iter().map(|v| v.as_i64().and_then(|x| usize::try_from(x).ok())).collect()
}

/// `out[order[i]] = b[i]` (scatter, as `split` uses) - `None` unless `order` is a permutation of `b`'s indices.
fn scatter(b: &[u8], order: &[usize]) -> Option<Vec<u8>> {
    let mut out: Vec<Option<u8>> = vec![None; b.len()];
    if order.len() != b.len() {
        return None;
    }
    for (&c, &pos) in b.iter().zip(order) {
        let slot = out.get_mut(pos)?;
        if slot.replace(c).is_some() {
            return None;
        }
    }
    out.into_iter().collect()
}

/// `out[i] = b[order[i]]` (gather).
fn gather(b: &[u8], order: &[usize]) -> Option<Vec<u8>> {
    (order.len() == b.len()).then(|| order.iter().map(|&i| b.get(i).copied()).collect()).flatten()
}

/// d29 `table`: `data[i] = p[i] + key + step*i` (mod 256). The offset and step are derived from the known
/// `ANS-` prefix (must form an arithmetic progression over its four bytes); `None` otherwise.
fn untable(b: &[u8]) -> Option<Vec<u8>> {
    if b.len() < 4 {
        return None;
    }
    let d: Vec<u8> = b.iter().zip(b"ANS-").map(|(x, p)| x.wrapping_sub(*p)).collect();
    let step = d[1].wrapping_sub(d[0]);
    if (1..4).any(|i| d[i].wrapping_sub(d[i - 1]) != step) {
        return None;
    }
    Some(b.iter().enumerate().map(|(i, x)| x.wrapping_sub(d[0].wrapping_add(step.wrapping_mul(i as u8)))).collect())
}

/// XOR counterpart of `untable`: `data[i] = p[i] ^ (k + step*i)`, with `k` and `step` recovered from the `ANS-` prefix.
fn unxor_ramp(b: &[u8]) -> Option<Vec<u8>> {
    if b.len() < 4 {
        return None;
    }
    let d: Vec<u8> = b.iter().zip(b"ANS-").map(|(x, p)| x ^ p).collect();
    let step = d[1].wrapping_sub(d[0]);
    if step == 0 || (1..4).any(|i| d[i].wrapping_sub(d[i - 1]) != step) {
        return None;
    }
    Some(b.iter().enumerate().map(|(i, x)| x ^ d[0].wrapping_add(step.wrapping_mul(i as u8))).collect())
}

/// d30 `layered`: base64, then byte `i` XOR `(key + i)`, then reversed.
fn unlayer(data: &str, key: i64) -> Option<Vec<u8>> {
    let b = codec::base64(data)?;
    Some(b.iter().enumerate().map(|(i, x)| x ^ (key as u8).wrapping_add(i as u8)).rev().collect())
}

fn xor_with(b: &[u8], key: &[u8]) -> Vec<u8> {
    b.iter().enumerate().map(|(i, x)| x ^ key[i % key.len()]).collect()
}

/// `java.util.Random(seed)`: the classic 48-bit LCG.
struct JavaRandom(u64);

impl JavaRandom {
    fn new(seed: i64) -> Self {
        JavaRandom((seed as u64 ^ 0x5DEE_CE66D) & ((1 << 48) - 1))
    }
    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(0x5DEE_CE66D).wrapping_add(0xB)) & ((1 << 48) - 1);
        (self.0 >> (48 - bits)) as i32
    }
    fn next_bytes(&mut self, n: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(n);
        while out.len() < n {
            let mut r = self.next(32);
            for _ in 0..(n - out.len()).min(4) {
                out.push(r as u8);
                r >>= 8;
            }
        }
        out
    }
    fn next_int_256(&mut self) -> u8 {
        ((256i64 * self.next(31) as i64) >> 31) as u8
    }
}

/// Candidate keystreams derived from integer seeds in the puzzle.
fn seeded_streams(p: &Value, n: usize) -> Vec<Vec<u8>> {
    let mut seeds: Vec<i64> = Vec::new();
    for name in ["seed", "nonce", "iv", "salt"] {
        if let Some(v) = p.get(name).and_then(Value::as_i64) {
            seeds.push(v);
        }
    }
    let mut out = Vec::new();
    for s in seeds {
        out.push(JavaRandom::new(s).next_bytes(n));
        let mut r = JavaRandom::new(s);
        out.push((0..n).map(|_| r.next_int_256()).collect());
    }
    out
}

/// Bounded breadth-first search over byte transforms of `data` / joined `parts`.
fn generic(p: &Value) -> Result<String, String> {
    let ks = keys(p);
    let ord = order(p);
    let mut frontier: Vec<Vec<u8>> = Vec::new();
    if let Some(d) = p.str_of("data") {
        frontier.push(d.as_bytes().to_vec());
    }
    if let Some(parts) = p.get("parts").and_then(Value::as_array) {
        let joined: String = parts.iter().filter_map(Value::as_str).collect();
        frontier.push(joined.into_bytes());
    }
    let mut seen: std::collections::HashSet<Vec<u8>> = frontier.iter().cloned().collect();
    let mut hits: std::collections::BTreeSet<String> = Default::default();
    for depth in 0..=3 {
        for v in &frontier {
            hits.extend(ans_codes(v));
        }
        if !hits.is_empty() || depth == 3 {
            break;
        }
        let mut next = Vec::new();
        for v in &frontier {
            let s = String::from_utf8_lossy(v).into_owned();
            let mut outs: Vec<Vec<u8>> = vec![v.iter().rev().copied().collect()];
            if let Some(o) = &ord {
                outs.extend(scatter(v, o));
                outs.extend(gather(v, o));
            }
            outs.extend(untable(v));
            outs.extend(unxor_ramp(v));
            outs.extend(codec::hex(&s));
            outs.extend(codec::base64(&s));
            outs.extend(codec::base32(&s));
            if std::str::from_utf8(v).is_ok() {
                outs.push(codec::rot(&s, 13).into_bytes());
            }
            for &k in &ks {
                outs.push(xor_with(v, &[k as u8]));
                outs.push(v.iter().map(|x| x.wrapping_sub(k as u8)).collect());
                outs.push(v.iter().map(|x| x.wrapping_add(k as u8)).collect());
            }
            if ks.len() > 1 {
                outs.push(xor_with(v, &ks.iter().map(|&k| k as u8).collect::<Vec<_>>()));
            }
            for ks in seeded_streams(p, v.len()) {
                outs.push(xor_with(v, &ks));
            }
            for o in outs {
                if !o.is_empty() && seen.insert(o.clone()) {
                    next.push(o);
                }
            }
        }
        frontier = next;
    }
    match hits.len() {
        1 => Ok(hits.into_iter().next().unwrap()),
        0 => Err(format!("codec {:?}: no ANS- code found; puzzle {}", p.str_of("codec"), trunc(&p.to_json()))),
        _ => Err(format!("codec {:?}: several candidates {hits:?}", p.str_of("codec"))),
    }
}

fn trunc(s: &str) -> String {
    s.chars().take(300).collect()
}

/// Head and tail of a nested error chain (the innermost cause is at the end).
fn ends(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    if c.len() <= 400 {
        return s.to_string();
    }
    format!("{} ... {}", c[..100].iter().collect::<String>(), c[c.len() - 300..].iter().collect::<String>())
}

/// Decodes a puzzle object.
pub fn decode(p: &Value) -> Result<String, String> {
    let data = p.str_of("data").unwrap_or("");
    let key = p.get("key").and_then(Value::as_i64);
    let direct: Option<Vec<u8>> = match p.str_of("codec").unwrap_or("xor") {
        "split" => {
            // parts[i] belongs at position order[i].
            let parts = p.get("parts").and_then(Value::as_array).ok_or("split without parts")?;
            let order = p.get("order").and_then(Value::as_array).ok_or("split without order")?;
            let mut out = vec![String::new(); parts.len()];
            for (part, pos) in parts.iter().zip(order) {
                let pos = pos.as_i64().ok_or("bad order")? as usize;
                *out.get_mut(pos).ok_or("order out of range")? = part.as_str().ok_or("bad part")?.to_string();
            }
            Some(out.concat().into_bytes())
        }
        "xor" => match (codec::base64(data), &p.get("key")) {
            (Some(raw), Some(Value::Arr(a))) => {
                let k: Vec<u8> = a.iter().filter_map(Value::as_i64).map(|x| x as u8).collect();
                (!k.is_empty()).then(|| xor_with(&raw, &k))
            }
            (Some(raw), _) => key.map(|k| xor_with(&raw, &[k as u8])),
            _ => None,
        },
        // Characters of `data` permuted: data[i] belongs at position order[i].
        "indexed" => order(p).and_then(|o| scatter(data.as_bytes(), &o)),
        "hex" => codec::hex(data),
        // d29: hex bytes shifted by `key` + a per-position step (offset/step recovered from the `ANS-` prefix).
        "table" => codec::hex(data).and_then(|b| untable(&b)),
        "layered" => key.and_then(|k| unlayer(data, k)),
        "reverse" => Some(codec::reverse(data).into_bytes()),
        "base64" => codec::base64(data),
        "caesar" => key.map(|k| data.chars().map(|c| (c as i64 - k).rem_euclid(256) as u8).collect()),
        _ => None,
    };
    match direct.as_deref().and_then(is_answer) {
        Some(a) => Ok(a),
        None => generic(p),
    }
}

/// Answers found in one delivery, by APK name.
pub struct Apks {
    pub items: Vec<(String, Result<String, String>)>,
}

/// The single `ANS-` code across the named APK members; `Ok(None)` when they carry none.
fn unique_code<'a>(z: &Zip, names: impl Iterator<Item = &'a str>, what: &str) -> Result<Option<String>, String> {
    let mut codes = std::collections::BTreeSet::new();
    for name in names {
        codes.extend(ans_codes(&z.read_name(name, b"infected")?));
    }
    match codes.len() {
        0 => Ok(None),
        1 => Ok(codes.into_iter().next()),
        _ => Err(format!("{what}: several candidates {codes:?}")),
    }
}

/// d26 split APKs (`.apks` = `base.apk` + configuration/feature splits): a split's `assets/*.json`
/// (`{"part":..,"part_index":i,..}`) carries the base puzzle's missing `parts[i]`.
fn split_fragments(z: &Zip) -> Result<Vec<(usize, String)>, String> {
    let mut out = Vec::new();
    for f in z.files().filter(|f| f.name.starts_with("assets/") && f.name.ends_with(".json")) {
        let Ok(v) = Value::parse_bytes(&z.read(f, b"infected")?) else { continue };
        if let (Some(part), Some(i)) = (v.str_of("part"), v.get("part_index").and_then(Value::as_i64)) {
            out.push((usize::try_from(i).map_err(|_| format!("{}: bad part_index {i}", f.name))?, part.to_string()));
        }
    }
    Ok(out)
}

/// The puzzle with `parts[i]` filled from the splits' fragments (only empty slots; a conflicting one is an error).
fn merge_fragments(v: Value, frags: &[(usize, String)]) -> Result<Value, String> {
    if frags.is_empty() {
        return Ok(v);
    }
    let Value::Obj(mut members) = v else { return Err("puzzle is not an object".into()) };
    let parts = members.iter_mut().find(|(k, _)| k == "parts").map(|(_, p)| p).ok_or("split fragments but no parts")?;
    let Value::Arr(parts) = parts else { return Err("parts is not an array".into()) };
    for (i, part) in frags {
        let slot = parts.get_mut(*i).ok_or_else(|| format!("part_index {i} out of range"))?;
        match slot.as_str() {
            Some("") | None => *slot = Value::Str(part.clone()),
            Some(old) if old == part => {}
            Some(old) => return Err(format!("parts[{i}]: base has {old:?}, split has {part:?}")),
        }
    }
    Ok(Value::Obj(members))
}

/// Solves a base APK; `splits` are the other APKs of its split set (d26 `.apks`), whose assets, native
/// libraries and part fragments belong to the same installed app.
fn solve_apk(bytes: &[u8], splits: &[(String, Zip)]) -> Result<String, String> {
    let z = Zip::parse(bytes)?;
    let raw = z.read_name(PUZZLE, b"infected")?;
    let mut frags = Vec::new();
    for (_, s) in splits {
        frags.extend(split_fragments(s)?);
    }
    let v = merge_fragments(Value::parse_bytes(&raw).map_err(|e| format!("{PUZZLE}: {e}"))?, &frags)?;
    // A puzzle string naming an APK member (d20 `dynamic`: `data` = `assets/payload.dex`, a secondary DEX
    // whose string table holds the code).
    let members: Vec<&str> = v.as_object().unwrap_or_default().iter().filter_map(|(_, x)| x.as_str()).filter(|s| *s != PUZZLE && z.find(s).is_some()).collect();
    if let Some(a) = unique_code(&z, members.iter().copied(), "referenced members")? {
        return Ok(a);
    }
    decode(&v).or_else(|e| {
        // Native libraries (d15/16 carried the code in `libarena.so` too).
        let libs: Vec<String> = z.files().filter(|f| f.name.starts_with("lib/") && f.name.ends_with(".so")).map(|f| f.name.clone()).collect();
        if let Some(a) = unique_code(&z, libs.iter().map(String::as_str), "lib/*.so")? {
            return Ok(a);
        }
        // d23 `analysis`: no data at all - the code is computed by `classes.dex` (`Release.code(Context)`
        // through a chain of long mixers); interpret it.
        dex_code(&z, splits).map_err(|d| format!("{e}; dex: {d}"))
    })
}

/// The single `ANS-` code returned by the static `String` methods of the APK's DEX files, run in the interpreter.
fn dex_code(z: &Zip, splits: &[(String, Zip)]) -> Result<String, String> {
    let mut codes = std::collections::BTreeMap::<String, Vec<String>>::new();
    let mut seen = Vec::new();
    // d15 `analysis`: a `native` mixer in `lib/x86_64/libarena.so` (JNI symbol, straight-line arithmetic).
    let mut libs: Vec<(Vec<u8>, Vec<(String, usize)>)> = Vec::new();
    let apks = || std::iter::once(z).chain(splits.iter().map(|(_, s)| s));
    for (a, f) in apks().flat_map(|a| a.files().map(move |f| (a, f))).filter(|(_, f)| f.name.starts_with("lib/x86_64/") && f.name.ends_with(".so")) {
        let b = a.read(f, b"infected")?;
        let Some(e) = Elf::parse(&b) else { continue };
        let syms = e.symbols(&b, 11).unwrap_or_default().into_iter().filter(|s| s.value != 0).filter_map(|s| Some((s.name, e.vaddr_to_offset(s.value)?))).collect();
        libs.push((b, syms));
    }
    let native = |sym: &str, args: &[u64]| -> Result<u64, String> {
        let (b, off) = libs.iter().find_map(|(b, syms)| syms.iter().find(|(n, _)| n == sym).map(|(_, o)| (b, *o))).ok_or_else(|| format!("no x86_64 symbol {sym}"))?;
        let mut full = vec![0, 0];
        full.extend_from_slice(args);
        x64emu::call(b, off, &full)
    };
    // Split assets are merged into the app's AssetManager (d26 feature split).
    // `ApplicationInfo.splitSourceDirs` + `ZipFile` reads go through `dalvik::SPLITS` / `<split>!/<member>`.
    let files = |name: &str| {
        if name == dalvik::SPLITS {
            return Some(splits.iter().map(|(n, _)| format!("{n}\n")).collect::<String>().into_bytes());
        }
        if let Some((path, member)) = name.split_once("!/") {
            return splits.iter().find(|(n, _)| n == path).and_then(|(_, s)| s.read_name(member, b"infected").ok());
        }
        apks().find_map(|a| a.read_name(name, b"infected").ok())
    };
    // Only the primary DEX files are entry points; others (assets/payload.dex) load through class loaders.
    for f in z.files().filter(|f| f.name.ends_with(".dex") && !f.name.contains('/')) {
        let dex = std::rc::Rc::new(Dex::parse(&z.read(f, b"infected")?).map_err(|e| format!("{}: {e}", f.name))?);
        for (sig, r) in dalvik::run_string_methods(&dex, Some(&native), Some(&files)) {
            match &r {
                Ok(s) => ans_codes(s.as_bytes()).into_iter().for_each(|c| codes.entry(c).or_default().push(sig.clone())),
                Err(e) => seen.push(format!("{sig}: {}", ends(e))),
            }
        }
    }
    match codes.len() {
        1 => Ok(codes.into_keys().next().unwrap()),
        0 => Err(format!("no ANS- code from the static String methods; failures {seen:?}")),
        _ => Err(format!("several candidates {codes:?}")),
    }
}

fn collect(name: &str, bytes: &[u8], out: &mut Vec<(String, Result<String, String>)>, depth: u8) -> Result<(), String> {
    let z = Zip::parse(bytes)?;
    if z.find(PUZZLE).is_some() {
        out.push((name.to_string(), solve_apk(bytes, &[])));
        return Ok(());
    }
    if depth > 3 {
        return Err("archives nested too deep".into());
    }
    // A container of APKs (or of zips of APKs).
    let mut inner = Vec::new();
    for e in z.files() {
        let b = z.read(e, b"infected")?;
        if zip::is_zip(&b) {
            inner.push((e.base_name().to_string(), b));
        }
    }
    // A split-APK set (d26 `.apks`): one base APK with the puzzle plus split APKs (a manifest, no puzzle).
    // Answered as one item named after the set (the question's artifact).
    let mut bases = Vec::new();
    let mut splits = Vec::new();
    for (n, b) in &inner {
        let iz = Zip::parse(b)?;
        if iz.find(PUZZLE).is_some() {
            bases.push(b);
        } else if iz.find("AndroidManifest.xml").is_some() {
            splits.push((n.clone(), iz));
        }
    }
    if !splits.is_empty() {
        match bases.as_slice() {
            [base] => out.push((name.to_string(), solve_apk(base, &splits))),
            _ => return Err(format!("{name}: {} split APKs with {} base APKs", splits.len(), bases.len())),
        }
        return Ok(());
    }
    for (n, b) in &inner {
        collect(n, b, out, depth + 1)?;
    }
    Ok(())
}

impl Lane for Mobile {
    fn name(&self) -> &'static str {
        "mobile"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["mobile-android-day-"]
    }

    fn analyse(&self, d: &Delivery, _: &RunCtx) -> Result<Analysis, String> {
        let mut items = Vec::new();
        collect(&d.name, &d.bytes, &mut items, 0)?;
        if items.is_empty() {
            return Err(format!("{}: no APK with {PUZZLE}", d.name));
        }
        Ok(Box::new(Apks { items }))
    }

    fn answer(&self, q: &Question, done: &Analysed, _: &RunCtx) -> Result<String, String> {
        let name = q.artifact_path().last().map(|s| s.to_string());
        let pick = |a: &Apks| -> Option<Result<String, String>> {
            if a.items.len() == 1 {
                return Some(a.items[0].1.clone());
            }
            a.items.iter().find(|(n, _)| Some(n) == name.as_ref()).map(|(_, r)| r.clone())
        };
        if let Some((n, _)) = q.delivery_label() {
            let a = done.get::<Apks>(n.checked_sub(1).ok_or("delivery 0")?)?;
            return pick(a).ok_or_else(|| format!("delivery {n} has no APK {name:?}"))?;
        }
        let all: Vec<&(String, Result<String, String>)> = done.all::<Apks>().flat_map(|(_, a)| a.items.iter()).collect();
        let named: Vec<_> = all.iter().filter(|(n, _)| Some(n) == name.as_ref()).collect();
        match (named.as_slice(), all.as_slice()) {
            ([one], _) => one.1.clone(),
            ([], [only]) => only.1.clone(),
            _ => Err(format!("cannot match artifact {:?} to one APK", q.artifact)),
        }
    }

    fn self_validating(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Result<String, String> {
        decode(&Value::parse(s).unwrap())
    }

    #[test]
    fn codecs() {
        assert_eq!(d(r#"{"codec":"split","parts":["0A353BE7B","ANS-5","709E18"],"order":[2,0,1]}"#).unwrap(), "ANS-5709E180A353BE7B");
        let indexed = r#"{"codec":"indexed","data":"-2FE1A2D163AA1N51SC0","order":[3,10,17,4,11,18,5,12,19,6,13,0,7,14,1,8,15,2,9,16]}"#;
        assert_eq!(d(indexed).unwrap(), "ANS-E26A5C21D3110FA1");
        // Unknown name, same permutation: found by the search.
        assert_eq!(d(&indexed.replace("indexed", "shuffle")).unwrap(), "ANS-E26A5C21D3110FA1");
        assert_eq!(d(r#"{"codec":"reverse","data":"6667D24E8BC6DE39-SNA"}"#).unwrap(), "ANS-93ED6CB8E42D7666");
        assert_eq!(d(r#"{"codec":"caesar","data":"KX]7BKPOK?>:CCP=;:@O","key":10}"#).unwrap(), "ANS-8AFEA54099F3106E");
        assert_eq!(d(r#"{"codec":"hex","data":"414e532d31323334353637383941424344454630"}"#).unwrap(), "ANS-123456789ABCDEF0");
        // d29 `table`: hex bytes + key + 3*i (graded CORRECT); also found by the search under another name.
        let table = r#"{"codec":"table","data":"6a7a825f767a6c82787c8b8d8592948a9d9597a5","key":41}"#;
        assert_eq!(d(table).unwrap(), "ANS-AB1D78DC8BA4D98C");
        assert_eq!(d(&table.replace("\"table\"", "\"lut\"")).unwrap(), "ANS-AB1D78DC8BA4D98C");
        // d30 `layered`: base64 -> xor (key + i) -> reverse; also found by the search (ramp from the prefix).
        let layered = r#"{"codec":"layered","data":"TU49OyInJlUlJSQkKilcKDFOUF4=","key":12}"#;
        assert_eq!(d(layered).unwrap(), "ANS-3F023201F46243CA");
        assert_eq!(d(&layered.replace("\"layered\"", "\"onion\"")).unwrap(), "ANS-3F023201F46243CA");
        // Unknown codec: base64 of the reversed answer, found by search.
        let enc = {
            let r: String = "ANS-123456789ABCDEF0".chars().rev().collect();
            let bytes = r.as_bytes();
            let t = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut s = String::new();
            for c in bytes.chunks(3) {
                let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
                for k in 0..4 {
                    if k <= c.len() {
                        s.push(t[(n >> (18 - 6 * k) & 63) as usize] as char);
                    }
                }
            }
            s
        };
        assert_eq!(d(&format!(r#"{{"codec":"mystery","data":"{enc}"}}"#)).unwrap(), "ANS-123456789ABCDEF0");
        assert!(d(r#"{"codec":"mystery","data":"nothing"}"#).is_err());
    }

    #[test]
    fn analysis_apks_run_their_dex() {
        let apk = |n: &str| std::fs::read(format!("{}/testdata/{n}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        // d23: `Release.code` over long mixers and brute-force `Probe` loops (graded CORRECT, also checked by hand).
        assert_eq!(solve_apk(&apk("d23-mobile-tt.apk"), &[]).unwrap(), "ANS-E4F679E90A7D1CAE");
        // v2 d16: guarded JNI `nativeShare(JI)J` in lib/x86_64/libarena.so (cmp/jcc/movzx).
        assert_eq!(solve_apk(&apk("d23-mobile-v2-d16.apk"), &[]).unwrap(), "ANS-7BF9CD9FEB8E911F");
        // v2 d20: assets/payload.dex through InMemoryDexClassLoader + reflection.
        assert_eq!(solve_apk(&apk("d23-mobile-v2-d20.apk"), &[]).unwrap(), "ANS-02D5DB6B306ACAF5");
    }

    #[test]
    fn split_apk_sets() {
        let set = |n: &str| {
            let b = std::fs::read(format!("{}/testdata/{n}", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let mut out = Vec::new();
            collect(n, &b, &mut out, 0).unwrap();
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].0, n);
            out.remove(0).1
        };
        // d26 v1 `split`: the feature split's `assets/huntress_feature.json` fills the base's empty `parts[0]`.
        assert_eq!(set("d26-mobile-single.apks").unwrap(), "ANS-7E37D134754DEA01");
        // d26 v2: `Assets.readSplit` reads `huntress_feature.bin` via `splitSourceDirs` + `ZipFile` (graded CORRECT).
        assert_eq!(set("d26-mobile-tt.apks").unwrap(), "ANS-2DF27FDEEFEF5A67");
    }

    #[test]
    fn java_random_matches_jdk() {
        // JDK: new Random(42).nextInt() == -1170105035 (0xBA419D35), so nextBytes(4) == 35 9d 41 ba.
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next(32), -1170105035);
        assert_eq!(JavaRandom::new(42).next_bytes(4), [0x35, 0x9d, 0x41, 0xba]);
        // xor with a seeded stream is found by the search.
        let ks = JavaRandom::new(1234).next_bytes(20);
        let enc: Vec<u8> = xor_with(b"ANS-123456789ABCDEF0", &ks);
        let hexs: String = enc.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(d(&format!(r#"{{"codec":"prng_xor","data":"{hexs}","seed":1234}}"#)).unwrap(), "ANS-123456789ABCDEF0");
    }
}
