//! Binary reverse engineering: an evidence zip holding one specimen per day (`dNN--variant-NNN.ext`,
//! cumulative: d01..dNN), each in a different language/container, all carrying the same HCFG config
//! blob. Questions are `dNN_<artifact|analysis>_<field>`.
//!
//! Every specimen gets the same generic analysis (container, header fields, sections, symbols, HCFG
//! wherever it hides), so a new day usually needs nothing. Formats follow the question's own
//! `answer_format`: `exact_text` containers are lowercase (`elf`), `pattern` ones canonical (`ELF`);
//! an `integer` ELF class is `64`, otherwise `ELF64`.

use super::{Analysed, Analysis, Lane};
use crate::crypto::sha256::{hex, sha256};
use crate::formats::{bytes, dotnet, elf, hcfg, jvm, macho, pe, x86, zip};
use crate::json::Value;
use crate::model::{Delivery, Question, RunCtx};
use std::collections::BTreeMap;

pub struct BinaryRe;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Elf,
    Pe,
    Jar,
    MachO,
    Other,
}

#[derive(Debug, Default)]
pub struct Spec {
    pub name: String,
    pub sha256: String,
    pub container: Option<Container>,
    pub machine: Option<&'static str>,
    pub elf_class: Option<u8>,
    pub big_endian: bool,
    pub elf_type: Option<u16>,
    pub entry: Option<u64>,
    pub has_symtab: bool,
    pub section_names: Vec<String>,
    pub symbols: Vec<String>,
    /// recover_code disassembly (`Ok(vec![])` when there is no such symbol).
    pub disassembly: Option<Result<Vec<String>, String>>,
    pub has_hcfg_section: bool,
    pub hcfg: Option<hcfg::Hcfg>,
    /// Headerless single-byte-XOR config (the .NET specimen's older encoding).
    pub xor_config: Option<Value>,
    pub dotnet_version: Option<String>,
    pub is_dotnet: bool,
    pub cpp_typeinfo: bool,
    pub cpp_vtable: bool,
    pub zip_methods: Vec<u16>,
}

/// One delivery: specimens grouped by day (`d01` -> primary specimen + sidecar files).
pub struct Set {
    pub days: BTreeMap<String, (Spec, Vec<(String, String)>)>,
}

fn has_mangled(d: &[u8], prefix: &[u8]) -> bool {
    bytes::find_all(d, prefix).into_iter().any(|i| d.get(i + prefix.len()).is_some_and(u8::is_ascii_digit))
}

fn jar_hcfg(d: &[u8]) -> Option<hcfg::Hcfg> {
    let z = zip::Zip::parse(d).ok()?;
    let mut classes: Vec<&zip::Entry> = z.files().filter(|e| e.name.ends_with(".class")).collect();
    // App classes before bundled runtimes (a Kotlin fat jar has ~3000 members).
    classes.sort_by_key(|e| (["kotlin/", "kotlinx/", "org/", "META-INF/"].iter().any(|p| e.name.starts_with(p)), e.name.clone()));
    for e in &classes {
        let Ok(c) = z.read(e, b"") else { continue };
        for u in jvm::utf8_constants(&c).unwrap_or_default() {
            if u.len() < 24 || !u.iter().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(b)) {
                continue;
            }
            if let Some(blob) = std::str::from_utf8(u).ok().and_then(crate::codec::base64) {
                if let Some(mut h) = hcfg::raw(&blob) {
                    h.source = "jar-constant";
                    return Some(h);
                }
            }
        }
    }
    // Fallback: raw blobs or base64 runs anywhere in the archive.
    for e in z.files() {
        let Ok(raw) = z.read(e, b"") else { continue };
        if let Some(h) = hcfg::raw(&raw).or_else(|| hcfg::any_base64(&raw)) {
            return Some(h);
        }
    }
    None
}

pub fn analyse_specimen(name: &str, d: &[u8]) -> Spec {
    let prof = std::env::var_os("CTF_PROFILE").is_some();
    let t = std::time::Instant::now();
    let mut lap = |what: &str| {
        if prof {
            eprintln!("  [profile] {name} {what} {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        }
    };
    // Hash large specimens on their own thread while the scans below run.
    std::thread::scope(|scope| {
        let hash = (d.len() > (4 << 20)).then(|| scope.spawn(|| hex(&sha256(d))));
        let mut s = analyse_specimen_body(name, d, &mut lap);
        s.sha256 = match hash {
            Some(h) => h.join().unwrap_or_default(),
            None => hex(&sha256(d)),
        };
        lap("sha256");
        s
    })
}

fn analyse_specimen_body(name: &str, d: &[u8], lap: &mut impl FnMut(&str)) -> Spec {
    let mut s = Spec { name: name.to_string(), ..Spec::default() };
    if let Some(e) = elf::Elf::parse(d) {
        s.container = Some(Container::Elf);
        s.machine = Some(elf::machine_name(e.machine));
        s.elf_class = Some(e.class);
        s.big_endian = e.endian == bytes::Endian::Big;
        s.elf_type = Some(e.kind);
        s.entry = Some(e.entry);
        s.section_names = e.sections.iter().map(|x| x.name.clone()).collect();
        s.has_hcfg_section = e.section(".hcfg").is_some();
        s.has_symtab = e.sections.iter().any(|x| x.kind == 2);
        let syms = e.symbols(d, 2).unwrap_or_default();
        s.symbols = syms.iter().filter(|x| !x.name.is_empty()).map(|x| x.name.clone()).collect();
        s.disassembly = Some(match syms.iter().find(|x| x.name == "recover_code") {
            None => Ok(Vec::new()),
            Some(sym) => match e.vaddr_to_offset(sym.value) {
                Some(off) => x86::disassemble(&d[off..d.len().min(off + sym.size.max(1) as usize)]),
                None => Err("recover_code address not in any section".into()),
            },
        });
    } else if let Some(p) = pe::Pe::parse(d) {
        s.container = Some(Container::Pe);
        s.machine = Some(pe::machine_name(p.machine));
        s.entry = Some(p.entry_rva as u64);
        s.section_names = p.sections.iter().map(|x| x.name.clone()).collect();
        s.has_hcfg_section = p.sections.iter().any(|x| x.name == ".hcfg");
        s.is_dotnet = p.is_dotnet();
    } else if let Some(m) = macho::parse(d) {
        s.container = Some(Container::MachO);
        s.machine = Some(macho::cpu_name(m.cputype));
        s.section_names = m.sections.iter().map(|(_, sect)| sect.clone()).collect();
        s.has_hcfg_section = m.sections.iter().any(|(_, sect)| sect.trim_start_matches('_') == "hcfg");
    } else if zip::is_zip(d) {
        s.container = Some(Container::Jar);
        if let Ok(z) = zip::Zip::parse(d) {
            let mut m: Vec<u16> = z.entries.iter().map(|e| e.method).collect();
            m.sort();
            m.dedup();
            s.zip_methods = m;
        }
    } else {
        s.container = Some(Container::Other);
    }
    lap("headers");
    if let Some(m) = dotnet::Metadata::find(d) {
        s.dotnet_version = Some(m.version.clone());
        s.is_dotnet = true;
    }
    lap("dotnet");
    s.cpp_typeinfo = has_mangled(d, b"_ZTI");
    s.cpp_vtable = has_mangled(d, b"_ZTV");
    lap("cpp");
    s.hcfg = if s.container == Some(Container::Jar) { jar_hcfg(d) } else { hcfg::raw(d).or_else(|| hcfg::base64_constant(d)) };
    if s.hcfg.is_none() || s.is_dotnet {
        s.xor_config = hcfg::xor_json(d).map(|(v, _, _)| v);
    }
    lap("hcfg");
    s
}

/// `d08--variant-000.elf` -> `d08`.
fn day_of(name: &str) -> Option<&str> {
    let (d, _) = name.split_once("--")?;
    (d.len() == 3 && d.starts_with('d') && d[1..].bytes().all(|c| c.is_ascii_digit())).then_some(d)
}

fn analyse_set(z: &zip::Zip) -> Result<Set, String> {
    // Group members by day; the primary specimen is the first non-.json file (sorted).
    let mut groups: BTreeMap<String, Vec<&zip::Entry>> = BTreeMap::new();
    for e in z.files() {
        if let Some(day) = day_of(e.base_name()) {
            groups.entry(day.to_string()).or_default().push(e);
        }
    }
    for g in groups.values_mut() {
        g.sort_by(|a, b| a.base_name().cmp(b.base_name()));
    }
    // Biggest specimens first, spread over two workers (the jumpbox has 2 cores).
    let mut jobs: Vec<(String, &zip::Entry, bool)> = Vec::new();
    for (day, g) in &groups {
        let primary = g.iter().position(|e| !e.base_name().ends_with(".json")).ok_or_else(|| format!("{day}: no specimen"))?;
        for (i, e) in g.iter().enumerate() {
            jobs.push((day.clone(), e, i == primary));
        }
    }
    jobs.sort_by_key(|(_, e, _)| std::cmp::Reverse(e.usize));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<Vec<(String, bool, Result<Spec, String>)>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..2)
            .map(|_| {
                s.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((day, e, primary)) = jobs.get(i) else { break };
                        let r = z.read(e, b"infected").map(|data| {
                            if *primary { analyse_specimen(e.base_name(), &data) } else { Spec { name: e.base_name().to_string(), sha256: hex(&sha256(&data)), ..Spec::default() } }
                        });
                        out.push((day.clone(), *primary, r));
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap_or_default()).collect()
    });
    let mut days: BTreeMap<String, (Option<Spec>, Vec<(String, String)>)> = BTreeMap::new();
    for (day, primary, r) in results.into_iter().flatten() {
        let spec = r?;
        let slot = days.entry(day).or_insert((None, Vec::new()));
        if primary {
            slot.0 = Some(spec);
        } else {
            slot.1.push((spec.name, spec.sha256));
        }
    }
    let days = days
        .into_iter()
        .map(|(d, (p, mut side))| {
            side.sort();
            (d, (p.expect("primary specimen analysed"), side))
        })
        .collect();
    Ok(Set { days })
}

fn container_label(s: &Spec, q: &Question) -> Result<String, String> {
    let exact = q.format_kind.as_deref() == Some("exact_text");
    let syntax = q.format_syntax.as_deref().unwrap_or("");
    Ok(match s.container {
        Some(Container::Pe) if s.is_dotnet && (syntax.contains("container_runtime") || syntax.contains("managed")) => "PE32_dotnet".into(),
        Some(Container::Elf) => (if exact { "elf" } else { "ELF" }).into(),
        Some(Container::Pe) => (if exact { "pe" } else { "PE" }).into(),
        Some(Container::Jar) => (if exact { "jar" } else { "JAR" }).into(),
        Some(Container::MachO) if !exact => "Mach-O".into(),
        other => return Err(format!("no graded label known for container {other:?} (format {:?}: {syntax:?})", q.format_kind)),
    })
}

fn config_value(v: &Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_else(|| v.to_json())
}

fn answer_spec(field: &str, s: &Spec, side: &[(String, String)], q: &Question) -> Result<String, String> {
    let need = |o: Option<String>, what: &str| o.ok_or_else(|| format!("{}: no {what}", s.name));
    let h = || s.hcfg.as_ref().ok_or_else(|| format!("{}: no HCFG blob found", s.name));
    let json = |items: &[String]| Value::Arr(items.iter().map(|x| Value::Str(x.clone())).collect()).to_json();
    Ok(match field {
        "artifact_sha256_1" => s.sha256.clone(),
        f if f.starts_with("artifact_sha256_") => {
            let n: usize = f["artifact_sha256_".len()..].parse().map_err(|_| "bad sha256 index")?;
            side.get(n - 2).map(|(_, h)| h.clone()).ok_or_else(|| format!("{}: no file #{n}", s.name))?
        }
        "analysis_format" => container_label(s, q)?,
        "analysis_machine" | "analysis_architecture" => need(s.machine.map(str::to_string), "machine")?,
        "analysis_elf_class" => {
            let c = need(s.elf_class.map(|c| c.to_string()), "ELF class")?;
            let bits = if c == "2" { 64 } else { 32 };
            if q.format_kind.as_deref() == Some("integer") { bits.to_string() } else { format!("ELF{bits}") }
        }
        "analysis_endianness" => (if s.big_endian { "big" } else { "little" }).into(),
        "analysis_elf_type" => need(
            s.elf_type.map(|t| match t {
                1 => "REL",
                2 => "EXEC",
                3 => "DYN",
                4 => "CORE",
                _ => "UNKNOWN",
            }.to_string()),
            "ELF type",
        )?,
        "analysis_entry_point" if s.container == Some(Container::Elf) => format!("0x{:x}", s.entry.unwrap_or(0)),
        "analysis_has_symtab" => s.has_symtab.to_string(),
        "analysis_section_names" => json(&s.section_names),
        "analysis_symbols" => json(&s.symbols),
        "analysis_disassembly" => match &s.disassembly {
            Some(Ok(ins)) => json(ins),
            Some(Err(e)) => return Err(e.clone()),
            None => return Err(format!("{}: not an ELF", s.name)),
        },
        "analysis_has_hcfg_section" => s.has_hcfg_section.to_string(),
        "analysis_has_cpp_typeinfo" => s.cpp_typeinfo.to_string(),
        "analysis_has_cpp_vtable" => s.cpp_vtable.to_string(),
        "analysis_metadata_version" => need(s.dotnet_version.clone(), ".NET metadata")?,
        // Python's json.dumps default separators: "[0, 8]".
        "analysis_compression_methods" => format!("[{}]", s.zip_methods.iter().map(u16::to_string).collect::<Vec<_>>().join(", ")),
        "analysis_config_crc32_hex" => format!("0x{:08x}", h()?.crc),
        "analysis_xor_key_hex" => format!("0x{:02x}", h()?.key),
        "analysis_hcfg_version" => h()?.version.to_string(),
        "analysis_hcfg_offset" => h()?.offset.to_string(),
        f if f.starts_with("analysis_config_") => {
            let key = &f["analysis_config_".len()..];
            let from = |v: Option<&Value>| v.and_then(|c| c.get(key)).map(config_value);
            from(s.hcfg.as_ref().map(|h| &h.config))
                .or_else(|| from(s.xor_config.as_ref()))
                .ok_or_else(|| format!("{}: config has no {key:?}", s.name))?
        }
        other => return Err(format!("unknown field {other:?} for {} ({:?})", s.name, s.container)),
    })
}

impl Lane for BinaryRe {
    fn name(&self) -> &'static str {
        "binary-re"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["binary-reverse-engineering-day-"]
    }

    fn analyse(&self, d: &Delivery, _: &RunCtx) -> Result<Analysis, String> {
        let z = zip::Zip::parse(&d.bytes)?;
        // A delivery is either the evidence zip itself or an outer zip of per-set evidence zips.
        if z.files().any(|e| day_of(e.base_name()).is_some()) {
            return Ok(Box::new(vec![analyse_set(&z)?]));
        }
        let mut sets = Vec::new();
        for e in z.files().filter(|e| e.name.ends_with(".zip")) {
            let inner = z.read(e, b"infected")?;
            sets.push(analyse_set(&zip::Zip::parse(&inner)?)?);
        }
        if sets.is_empty() {
            return Err(format!("{}: no dNN specimens", d.name));
        }
        Ok(Box::new(sets))
    }

    fn answer(&self, q: &Question, done: &Analysed, _: &RunCtx) -> Result<String, String> {
        let (day, field) = q.key.split_once('_').ok_or_else(|| format!("unexpected id {:?}", q.key))?;
        // Mastery: `delivery N:` (or set N) picks the evidence set; otherwise the only one.
        let all: Vec<&Vec<Set>> = done.all::<Vec<Set>>().map(|(_, s)| s).collect();
        let set: &Set = if let Some((n, _)) = q.delivery_label() {
            let sets = done.get::<Vec<Set>>(n.checked_sub(1).ok_or("delivery 0")?)?;
            sets.first().ok_or("empty delivery")?
        } else {
            let flat: Vec<&Set> = all.iter().flat_map(|v| v.iter()).collect();
            match (flat.len(), q.set) {
                (1, _) => flat[0],
                (n, Some(k)) if k as usize <= n => flat[k as usize - 1],
                _ => return Err(format!("cannot pick an evidence set for {:?}", q.id)),
            }
        };
        let (spec, side) = set.days.get(day).ok_or_else(|| format!("no {day} specimen in the evidence"))?;
        answer_spec(field, spec, side, q)
    }
}
