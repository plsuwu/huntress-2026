//! Binary reverse engineering: an evidence zip holding one specimen per day (`dNN--variant-NNN.ext`,
//! cumulative: d01..dNN), each in a different language/container, all carrying the same HCFG config
//! blob. Questions are `dNN_<artifact|analysis>_<field>`.
//!
//! Every specimen gets the same generic analysis (container, header fields, sections, symbols, HCFG
//! wherever it hides), so a new day usually needs nothing. Formats follow the question's own
//! `answer_format`: `exact_text` containers are lowercase (`elf`), `pattern` ones canonical (`ELF`);
//! an `integer` ELF class is `64`, otherwise `ELF64`.

use super::{Analysed, Analysis, Lane, Stream};
use crate::crypto::sha256::{hex, sha256};
use crate::formats::{bytes, dotnet, elf, hcfg, jvm, macho, overlay, pe, wasm, x86, zip};
use crate::json::Value;
use crate::model::{Delivery, Question, RunCtx};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

pub struct BinaryRe;

/// Process-wide cap on members being inflated + analysed at once, across every set in flight (a mastery
/// analyses each delivery on its own fetch worker). Each set's biggest member (`d08--sample.elf`, 25 MB
/// compressed -> 67 MB) comes first, so 9 concurrent sets held ~830 MB of members on top of the 321 MB of
/// deliveries: the d25 mastery peaked at ~1.8 GB on 2 cores and was OOM-killed on the jumpbox (1.9 GB,
/// ~450 MB taken by other services). One slot per core costs no CPU time and keeps the peak at ~2 members.
static SLOTS: Slots = Slots { busy: Mutex::new(0), freed: Condvar::new() };

struct Slots {
    busy: Mutex<usize>,
    freed: Condvar,
}

struct Slot;

impl Slots {
    fn take(&self) -> Slot {
        let limit = std::thread::available_parallelism().map_or(2, |n| n.get()).max(1);
        let mut busy = self.busy.lock().unwrap_or_else(|e| e.into_inner());
        while *busy >= limit {
            busy = self.freed.wait(busy).unwrap_or_else(|e| e.into_inner());
        }
        *busy += 1;
        Slot
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        *SLOTS.busy.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        SLOTS.freed.notify_one();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Elf,
    Pe,
    Jar,
    MachO,
    Wasm,
    /// NekoVM bytecode (`NEKO` magic; Haxe `.n`, Day 30).
    Neko,
    Other,
}

#[derive(Debug, Default)]
pub struct Spec {
    pub name: String,
    pub sha256: String,
    pub container: Option<Container>,
    pub machine: Option<&'static str>,
    /// Raw header machine number (ELF e_machine, PE COFF Machine, Mach-O cputype).
    pub machine_num: Option<u32>,
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
    /// NUL-terminated strings behind ELF OBJECT symbols (`marker` -> `BR01-CATALOG-000`).
    pub objects: BTreeMap<String, String>,
    /// `eax` at the `ret` of `recover_code` (`mov eax,0x1fa4; ret` -> 8100).
    pub recover_code: Option<u64>,
    pub go_runtime: bool,
    pub rust_mangled: bool,
    /// Jar: the manifest's `Main-Class` declaration.
    pub main_class: Option<jvm::ClassInfo>,
    pub coroutine_runtime: bool,
    /// Name of the section holding the validated HCFG blob (or its base64 text).
    pub hcfg_section: Option<String>,
    /// Itanium-mangled names found outside an ELF symtab (MinGW PE: COFF string table).
    pub mangled_names: Vec<String>,
    /// Final validated packed overlay (`PK19`): (magic offset, file length - offset).
    pub overlay: Option<(usize, usize)>,
    /// Resource overlay (`RS20`): the HCFG resource length field.
    pub hcfg_resource_size: Option<usize>,
    /// Jar: parsed `.json` members by path (`crash/trace.json`, d26).
    pub jar_json: BTreeMap<String, Value>,
    /// ELF e_shnum (section-header entries, index 0 included).
    pub section_count: Option<usize>,
    /// PE: nonempty primary COFF symbol names in raw table order (auxiliary records skipped).
    pub coff_symbols: Vec<String>,
    /// Nonempty SHT_DYNSYM names (`symbols` holds the SHT_SYMTAB ones).
    pub dynsym_names: Vec<String>,
    /// ELF `.text`: 0x70..0x7f bytes plus 0x0f 0x80..0x8f pairs (d17 `conditional_branch_opcode_count`).
    pub cond_branch_opcodes: Option<usize>,
    /// Jar: central-directory entries ending `.class`.
    pub class_count: Option<usize>,
    /// .NET CLI header (MajorRuntimeVersion, MinorRuntimeVersion).
    pub cli_runtime: Option<(u16, u16)>,
    /// Final validated `PK19` overlay's packing keys (k1, k2).
    pub packing_keys: Option<(u8, u8)>,
    /// Which `NEEDLES` occur in the file bytes: (as Latin-1, as UTF-16LE).
    pub needles: BTreeMap<&'static str, (bool, bool)>,
    /// Last CRC-valid record per `HG??` magic (HCFG layout, whole JSON object): `HGRF` campaign graph (Day 30),
    /// `HGAU` gauntlet (Day 31).
    pub records: BTreeMap<String, hcfg::Hcfg>,
}

/// Texts the "true exactly when the final bytes contain X" predicates name (Day 28); a predicate naming
/// anything else is refused, since the bytes are not kept past analysis.
const NEEDLES: [&str; 5] = ["decoy", "NimString", "NimMain", "FPC", "__gnat"];

/// One delivery: specimens grouped by day (`d01` -> primary specimen + sidecar files).
pub struct Set {
    /// day -> (primary specimen, sidecar (name, sha256) pairs, every other specimen fully analysed).
    pub days: BTreeMap<String, (Spec, Vec<(String, String)>, Vec<Spec>)>,
    /// day -> every member's (name, sha256) in central-directory order (`structural_*`, Day 28).
    pub members: BTreeMap<String, Vec<(String, String)>>,
}

/// `prefix` followed by a digit (an Itanium-mangled name); stops at the first hit.
fn has_mangled(d: &[u8], prefix: &[u8]) -> bool {
    let mut i = 0;
    while let Some(p) = bytes::find(&d[i..], prefix) {
        let at = i + p;
        if d.get(at + prefix.len()).is_some_and(u8::is_ascii_digit) {
            return true;
        }
        i = at + 1;
    }
    false
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
    analyse_specimen_with(name, d, None)
}

/// `sha256`: the hex digest when it is already known (hashed while the member streamed in).
fn analyse_specimen_with(name: &str, d: &[u8], sha256_hex: Option<String>) -> Spec {
    let prof = std::env::var_os("CTF_PROFILE").is_some();
    let t = std::time::Instant::now();
    let mut lap = |what: &str| {
        if prof {
            eprintln!("  [profile] {name} {what} {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        }
    };
    // Hash large specimens on their own thread while the scans below run.
    std::thread::scope(|scope| {
        let hash = (sha256_hex.is_none() && d.len() > (4 << 20)).then(|| scope.spawn(|| hex(&sha256(d))));
        let mut s = analyse_specimen_body(name, d, &mut lap);
        s.sha256 = match (sha256_hex, hash) {
            (Some(h), _) => h,
            (None, Some(h)) => h.join().unwrap_or_default(),
            (None, None) => hex(&sha256(d)),
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
        s.machine_num = Some(e.machine as u32);
        s.elf_class = Some(e.class);
        s.big_endian = e.endian == bytes::Endian::Big;
        s.elf_type = Some(e.kind);
        s.entry = Some(e.entry);
        s.section_names = e.sections.iter().map(|x| x.name.clone()).collect();
        s.has_hcfg_section = e.section(".hcfg").is_some();
        s.has_symtab = e.sections.iter().any(|x| x.kind == 2);
        s.section_count = Some(e.sections.len());
        s.cond_branch_opcodes = e.section(".text").and_then(|x| e.data(d, x)).map(cond_branch_opcodes);
        let syms = e.symbols(d, 2).unwrap_or_default();
        s.symbols = syms.iter().filter(|x| !x.name.is_empty()).map(|x| x.name.clone()).collect();
        for x in syms.iter().filter(|x| x.info & 0xf == 1 && !x.name.is_empty() && x.size > 0 && x.size <= 4096) {
            let text = e.vaddr_to_offset(x.value).and_then(|o| bytes::cstr(d, o)).filter(|b| !b.is_empty() && b.len() as u64 <= x.size);
            if let Some(t) = text.filter(|b| b.iter().all(|c| (0x20..0x7f).contains(c))) {
                s.objects.insert(x.name.clone(), String::from_utf8_lossy(t).into_owned());
            }
        }
        // The marker is the `.huntress` section as UTF-8 minus trailing NULs (Day 19 wording; stripped d01 builds
        // have no `marker` symbol), else the string behind the `marker` OBJECT symbol.
        let section = e.section(".huntress").and_then(|x| e.data(d, x)).map(|b| &b[..b.iter().rposition(|&c| c != 0).map_or(0, |p| p + 1)]);
        if let Some(t) = section.filter(|b| !b.is_empty() && std::str::from_utf8(b).is_ok()) {
            s.objects.insert("marker".into(), String::from_utf8_lossy(t).into_owned());
        } else if !s.objects.contains_key("marker") {
            let text = e.section(".huntress").and_then(|x| e.data(d, x)).and_then(|b| bytes::cstr(b, 0)).filter(|b| !b.is_empty() && b.iter().all(|c| (0x20..0x7f).contains(c)));
            if let Some(t) = text {
                s.objects.insert("marker".into(), String::from_utf8_lossy(t).into_owned());
            }
        }
        let dynsyms = e.symbols(d, 11).unwrap_or_default();
        s.dynsym_names = dynsyms.iter().filter(|x| !x.name.is_empty()).map(|x| x.name.clone()).collect();
        s.rust_mangled = syms.iter().chain(&dynsyms).any(|x| is_rust_mangled(&x.name));
        s.go_runtime = e.sections.iter().any(|x| x.name == ".gopclntab" || x.name == ".go.buildinfo");
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
        s.machine_num = Some(p.machine as u32);
        s.entry = Some(p.entry_rva as u64);
        s.section_names = p.sections.iter().map(|x| x.name.clone()).collect();
        s.has_hcfg_section = p.sections.iter().any(|x| x.name == ".hcfg");
        s.is_dotnet = p.is_dotnet();
        s.section_count = Some(p.sections.len());
        s.coff_symbols = coff_symbols(d, p.pe_off);
        // IMAGE_COR20_HEADER: cb u32, MajorRuntimeVersion u16 @+4, MinorRuntimeVersion u16 @+6.
        s.cli_runtime = p.dirs.get(14).filter(|(rva, _)| *rva != 0).and_then(|(rva, _)| p.rva_to_offset(*rva)).and_then(|o| Some((bytes::u16le(d, o + 4)?, bytes::u16le(d, o + 6)?)));
        s.go_runtime = p.sections.iter().any(|x| x.name == ".gopclntab" || x.name == ".go.buildinfo");
    } else if let Some(m) = macho::parse(d) {
        s.container = Some(Container::MachO);
        s.machine = Some(macho::cpu_name(m.cputype));
        s.machine_num = Some(m.cputype as u32);
        s.section_names = m.sections.iter().map(|(_, sect)| sect.clone()).collect();
        s.has_hcfg_section = m.sections.iter().any(|(_, sect)| sect.trim_start_matches('_') == "hcfg");
    } else if let Some(w) = wasm::parse(d) {
        s.container = Some(Container::Wasm);
        s.machine = Some(w.arch());
        s.section_names = w.custom_names.clone();
        s.has_hcfg_section = w.custom_names.iter().any(|n| n.trim_start_matches('.') == "hcfg");
    } else if d.starts_with(b"NEKO") {
        s.container = Some(Container::Neko);
    } else if zip::is_zip(d) {
        s.container = Some(Container::Jar);
        if let Ok(z) = zip::Zip::parse(d) {
            let mut m: Vec<u16> = z.entries.iter().map(|e| e.method).collect();
            m.sort();
            m.dedup();
            s.zip_methods = m;
            s.class_count = Some(z.entries.iter().filter(|e| e.name.ends_with(".class")).count());
            s.coroutine_runtime = z.files().any(|e| e.name.starts_with("kotlin/coroutines/") || e.name.starts_with("kotlinx/coroutines/"));
            s.main_class = jar_main_class(&z);
            for e in z.files().filter(|e| e.name.ends_with(".json") && e.usize <= 1 << 20) {
                if let Some(v) = z.read(e, b"").ok().and_then(|b| Value::parse_bytes(&b).ok()) {
                    s.jar_json.insert(e.name.clone(), v);
                }
            }
        }
    } else {
        s.container = Some(Container::Other);
    }
    // Go embeds its build-info magic in every binary (PE/Mach-O builds have no `.gopclntab` section).
    s.go_runtime |= bytes::find(d, b"\xff Go buildinf:").is_some();
    s.recover_code = match &s.disassembly {
        Some(Ok(ins)) if !ins.is_empty() => eval_eax(ins),
        _ => None,
    };
    lap("headers");
    // CLI metadata only lives in PE images; scanning a 67 MB ELF for `BSJB` cost ~0.1 s per set.
    if let Some(m) = matches!(s.container, Some(Container::Pe | Container::Other)).then(|| dotnet::Metadata::find(d)).flatten() {
        s.dotnet_version = Some(m.version.clone());
        s.is_dotnet = true;
    }
    lap("dotnet");
    s.cpp_typeinfo = has_mangled(d, b"_ZTI");
    s.cpp_vtable = has_mangled(d, b"_ZTV");
    if s.symbols.is_empty() && s.cpp_typeinfo {
        s.mangled_names = scan_mangled(d);
    }
    lap("cpp");
    for n in NEEDLES {
        let wide: Vec<u8> = n.bytes().flat_map(|b| [b, 0]).collect();
        s.needles.insert(n, (bytes::find(d, n.as_bytes()).is_some(), bytes::find(d, &wide).is_some()));
    }
    lap("needles");
    // A packed (d19) or resource (d20) overlay carries the config: the HCFG record at offset 0 of the decoded bytes, ahead of any raw hit in the stub.
    let packed = (s.container != Some(Container::Jar)).then(|| overlay::last(d)).flatten();
    s.overlay = packed.as_ref().map(|o| (o.offset, d.len() - o.offset));
    s.hcfg_resource_size = packed.as_ref().and_then(|o| o.resource_size);
    s.packing_keys = packed.as_ref().filter(|o| o.resource_size.is_none() && o.magic.starts_with("PK")).map(|o| (o.k1, o.k2));
    s.hcfg = if let Some(o) = &packed {
        hcfg::at(&o.unpacked, 0).map(|mut h| {
            h.source = "overlay";
            h
        })
    } else if s.container == Some(Container::Jar) {
        jar_hcfg(d)
    } else {
        hcfg::raw(d).or_else(|| hcfg::base64_constant(d))
    };
    if s.hcfg.is_none() || s.is_dotnet {
        s.xor_config = hcfg::xor_json(d).map(|(v, _, _)| v);
    }
    if let (Some(h), Some(e)) = (&s.hcfg, (s.container == Some(Container::Elf)).then(|| elf::Elf::parse(d)).flatten()) {
        s.hcfg_section = e.sections.iter().find(|x| x.kind != 8 && x.size > 0 && (x.offset..x.offset + x.size).contains(&(h.offset as u64))).map(|x| x.name.clone());
    }
    s.records = hcfg::last_hg_records(d).into_iter().map(|(m, h)| (String::from_utf8_lossy(&m).into_owned(), h)).collect();
    lap("hcfg");
    s
}

/// Primary COFF symbol names (long names via the string table after the symbols), auxiliary records skipped.
fn coff_symbols(d: &[u8], pe_off: usize) -> Vec<String> {
    let (Some(ptr), Some(n)) = (bytes::u32le(d, pe_off + 12), bytes::u32le(d, pe_off + 16)) else { return Vec::new() };
    let (ptr, n) = (ptr as usize, n as usize);
    let Some(strtab) = n.checked_mul(18).and_then(|x| x.checked_add(ptr)).filter(|&t| ptr != 0 && t <= d.len()) else { return Vec::new() };
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let e = ptr + i * 18;
        let name = if bytes::u32le(d, e) == Some(0) {
            bytes::u32le(d, e + 4).and_then(|o| bytes::cstr(d, strtab + o as usize))
        } else {
            d.get(e..e + 8).map(|b| &b[..b.iter().position(|&c| c == 0).unwrap_or(8)])
        };
        if let Some(b) = name.filter(|b| !b.is_empty()) {
            out.push(String::from_utf8_lossy(b).into_owned());
        }
        i += 1 + d.get(e + 17).copied().unwrap_or(0) as usize;
    }
    out
}

/// One left-to-right scan: every 0x70..0x7f byte (Jcc rel8) and every 0x0f 0x80..0x8f pair (Jcc rel32).
fn cond_branch_opcodes(t: &[u8]) -> usize {
    let (mut i, mut n) = (0, 0);
    while i < t.len() {
        if (0x70..=0x7f).contains(&t[i]) {
            n += 1;
        } else if t[i] == 0x0f && t.get(i + 1).is_some_and(|b| (0x80..=0x8f).contains(b)) {
            n += 1;
            i += 1;
        }
        i += 1;
    }
    n
}

/// NUL-terminated `_Z...` strings, kept whole when a COFF grouped-section prefix precedes the name
/// (`.text$_ZNK15DiscoveryEngine5scoreEPKc`: graded verbatim on the d17 mastery MinGW PE), deduplicated in file order.
fn scan_mangled(d: &[u8]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for at in bytes::find_all(d, b"_Z") {
        let mut start = at;
        if at > 0 && d[at - 1] == b'$' {
            start = d[..at - 1].iter().rposition(|&c| c == 0).map_or(0, |p| p + 1);
        } else if at > 0 && d[at - 1] != 0 {
            continue;
        }
        let Some(name) = bytes::cstr(d, start) else { continue };
        if name.len() < 5 || !name.iter().all(|c| c.is_ascii_alphanumeric() || b"_.$@".contains(c)) {
            continue;
        }
        let n = String::from_utf8_lossy(name).into_owned();
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

/// Legacy (`_ZN...17h<16 hex>E`) or v0 (`_R...`) Rust symbol mangling.
fn is_rust_mangled(n: &str) -> bool {
    let n = n.split('@').next().unwrap_or(n);
    if let Some(r) = n.strip_prefix("_R") {
        return r.starts_with(|c: char| c.is_ascii_uppercase());
    }
    n.starts_with("_ZN")
        && n.len() > 21
        && n.ends_with('E')
        && n[n.len() - 20..n.len() - 17] == *"17h"
        && n[n.len() - 17..n.len() - 1].bytes().all(|b| b.is_ascii_hexdigit())
}

/// `eax` at the first `ret` of a straight-line disassembly (`mov eax,imm` / `xor eax,eax`).
fn eval_eax(ins: &[String]) -> Option<u64> {
    let mut eax = None;
    for i in ins {
        match i.as_str() {
            "ret" => return eax,
            "xor eax,eax" => eax = Some(0),
            m if m.starts_with("mov eax,0x") => eax = Some(u64::from_str_radix(&m["mov eax,0x".len()..], 16).ok()?),
            m if m.contains("eax") || m.contains("rax") => return None,
            _ => {}
        }
    }
    None
}

/// The manifest's `Main-Class`, parsed.
fn jar_main_class(z: &zip::Zip) -> Option<jvm::ClassInfo> {
    let mf = z.read_name("META-INF/MANIFEST.MF", b"").ok()?;
    let mf = String::from_utf8_lossy(&mf);
    let main = mf.lines().find_map(|l| l.strip_prefix("Main-Class:"))?.trim().replace('.', "/");
    jvm::class_info(&z.read_name(&format!("{main}.class"), b"").ok()?)
}

/// `_ZTI<class>` of the polymorphic class owning `method` (`_ZNK15DiscoveryEngine5scoreEPKc` -> `_ZTI15DiscoveryEngine`),
/// else the only class with a vtable.
/// The first name (in table/file order) for the chosen class's typeinfo; COFF `.rdata$` prefixes are kept.
fn typeinfo_symbol(syms: &[String]) -> Option<String> {
    let core = |n: &str| n.rsplit('$').next().unwrap_or(n).to_string();
    let mut classes: Vec<String> = Vec::new();
    for n in syms {
        let c = core(n);
        if c.starts_with("_ZTI") && c[4..].starts_with(|ch: char| ch.is_ascii_digit()) && !classes.contains(&c[4..].to_string()) {
            classes.push(c[4..].to_string());
        }
    }
    let with_vtable: Vec<&String> = classes.iter().filter(|c| syms.iter().any(|v| core(v).strip_prefix("_ZTV") == Some(c.as_str()))).collect();
    let class = match (classes.as_slice(), with_vtable.as_slice()) {
        ([one], _) => one,
        (_, [one]) => *one,
        _ => return None,
    };
    syms.iter().find(|n| core(n).strip_prefix("_ZTI") == Some(class.as_str())).cloned()
}

/// The first Itanium-mangled symbol whose name contains the identifier `ident` (`<len><ident>`).
fn mangled_symbol(syms: &[String], ident: &str) -> Option<String> {
    let tok = format!("{}{ident}", ident.len());
    syms.iter().find(|n| n.rsplit('$').next().unwrap_or(n).starts_with("_Z") && n.contains(&tok)).map(|n| n.split('@').next().unwrap_or(n).to_string())
}

/// `d08--variant-000.elf` -> `d08`.
fn day_of(name: &str) -> Option<&str> {
    let (d, _) = name.split_once("--")?;
    (d.len() == 3 && d.starts_with('d') && d[1..].bytes().all(|c| c.is_ascii_digit())).then_some(d)
}

/// The member a question names: "Use the [unique] dNN-- member whose basename ends X." or "Use the dNN-- X member".
fn named_member(text: &str, day: &str) -> Option<String> {
    let token = |s: &str| s.split_whitespace().next().map(|t| t.trim_end_matches(['.', ';', ',']).to_string()).filter(|t| !t.is_empty());
    for lead in ["Use the unique ", "Use the "] {
        let Some(rest) = text.split_once(&format!("{lead}{day}-- ")).map(|(_, r)| r) else { continue };
        if let Some(r) = rest.strip_prefix("member whose basename ends ") {
            return token(r);
        }
        let mut w = rest.split_whitespace();
        if let (Some(t), Some(m)) = (w.next(), w.next()) {
            if m.trim_end_matches([';', ',', '.']) == "member" {
                return token(t);
            }
        }
    }
    None
}

/// `d21--sample-plugin.cmxs` matches `plugin.cmxs` (whole basename suffix) and `plugin` (extensionless stem suffix).
fn member_matches(name: &str, m: &str) -> bool {
    let stem = name.rsplit_once('.').map_or(name, |(a, _)| a);
    name.ends_with(m) || stem.ends_with(m)
}

/// One member's spec, as a set needs it: a non-primary `.json` sidecar keeps only its name and hash.
fn member_spec(z: &zip::Zip, e: &zip::Entry, primary: bool) -> Result<Spec, String> {
    let _slot = SLOTS.take();
    z.read(e, b"infected").map(|data| {
        // Day 15 ships stripped + symbolized builds of one specimen: analyse every binary.
        if primary || !e.base_name().ends_with(".json") {
            analyse_specimen(e.base_name(), &data)
        } else {
            Spec { name: e.base_name().to_string(), sha256: hex(&sha256(&data)), ..Spec::default() }
        }
    })
}

/// `streamed`: members analysed while the archive downloaded, by local-header offset. Each is used only
/// when the central directory confirms it (same name and size at that offset); everything else is
/// analysed here, so the result never depends on what the stream managed to do.
fn analyse_set(z: &zip::Zip, mut streamed: HashMap<u64, Streamed>) -> Result<Set, String> {
    // Group members by day in central-directory order: `artifact_sha256_N` is the day's Nth member
    // (graded on d15: `-symbolized` precedes `-stripped` there), and the primary specimen is the
    // first non-.json one.
    let mut groups: BTreeMap<String, Vec<&zip::Entry>> = BTreeMap::new();
    let mut order: BTreeMap<String, usize> = BTreeMap::new();
    for (i, e) in z.files().enumerate() {
        if let Some(day) = day_of(e.base_name()) {
            groups.entry(day.to_string()).or_default().push(e);
            order.insert(e.base_name().to_string(), i);
        }
    }
    // Biggest specimens first, spread over two workers (the jumpbox has 2 cores).
    let mut jobs: Vec<(String, &zip::Entry, bool)> = Vec::new();
    let mut early = Vec::new();
    for (day, g) in &groups {
        let primary = g.iter().position(|e| !e.base_name().ends_with(".json")).ok_or_else(|| format!("{day}: no specimen"))?;
        for (i, e) in g.iter().enumerate() {
            match streamed.remove(&e.lh_offset) {
                Some(m) if m.name == e.raw_name && m.csize == e.csize => early.push((day.clone(), *e, i == primary, m.job)),
                _ => jobs.push((day.clone(), *e, i == primary)),
            }
        }
    }
    drop(streamed); // members the directory does not confirm: their threads finish on their own
    if std::env::var_os("CTF_PROFILE").is_some() {
        eprintln!("  [profile] set: {} of {} members streamed", early.len(), early.len() + jobs.len());
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
                        out.push((day.clone(), *primary, member_spec(z, e, *primary)));
                    }
                    out
                })
            })
            .collect();
        // Streamed members (a failed one is redone the ordinary way, so its error is the usual one).
        let mine: Vec<_> = early
            .into_iter()
            .map(|(day, e, primary, job)| {
                let r = match job.join() {
                    Ok(Ok(spec)) if primary || !e.base_name().ends_with(".json") => Ok(spec),
                    Ok(Ok(spec)) => Ok(Spec { name: spec.name, sha256: spec.sha256, ..Spec::default() }),
                    _ => member_spec(z, e, primary),
                };
                (day, primary, r)
            })
            .collect();
        let mut all: Vec<_> = hs.into_iter().map(|h| h.join().unwrap_or_default()).collect();
        all.push(mine);
        all
    });
    let mut days: BTreeMap<String, (Option<Spec>, Vec<(String, String)>, Vec<Spec>)> = BTreeMap::new();
    for (day, primary, r) in results.into_iter().flatten() {
        let spec = r?;
        let slot = days.entry(day).or_insert((None, Vec::new(), Vec::new()));
        if primary {
            slot.0 = Some(spec);
        } else {
            slot.1.push((spec.name.clone(), spec.sha256.clone()));
            slot.2.push(spec);
        }
    }
    let days: BTreeMap<_, _> = days
        .into_iter()
        .map(|(d, (p, mut side, mut others))| {
            side.sort_by_key(|(n, _)| order.get(n).copied());
            others.sort_by_key(|x| order.get(&x.name).copied());
            (d, (p.expect("primary specimen analysed"), side, others))
        })
        .collect();
    let members = days
        .iter()
        .map(|(d, (p, side, _))| {
            let mut all: Vec<(String, String)> = std::iter::once((p.name.clone(), p.sha256.clone())).chain(side.iter().cloned()).collect();
            all.sort_by_key(|(n, _)| order.get(n).copied());
            (d.clone(), all)
        })
        .collect();
    Ok(Set { days, members })
}

/// Bytes of streamed member copies waiting or being analysed, across every delivery in flight. They come
/// on top of the deliveries themselves (a mastery holds 9 x 36 MB), so past the budget a member is left
/// for `finish`, which reads it from the finished body: a time trial streams everything, a mastery
/// streams what fits.
static STREAM_BYTES: AtomicUsize = AtomicUsize::new(0);
const STREAM_BUDGET: usize = 96 << 20;

/// Members at least this big (inflated) are decoded while they download rather than after.
const PROGRESSIVE_MIN: usize = 16 << 20;

/// Decodes a member from the data chunks the stream forwards, hashing the output as it grows; the scans
/// (under a core slot) run once the last chunk is in. A closed channel before the end (failed download)
/// is an error, so `finish` redoes the member the ordinary way.
fn progressive_spec(mut m: zip::MemberStream, rx: std::sync::mpsc::Receiver<Vec<u8>>, name: &str) -> Result<Spec, String> {
    let mut digest = ring::digest::Context::new(&ring::digest::SHA256);
    let mut hashed = 0;
    // Socket reads arrive ~16 KB at a time: decode in batches of whatever has queued up (>= 256 KB).
    let mut batch = Vec::new();
    let mut open = true;
    while open {
        match rx.recv() {
            Ok(c) => batch.extend_from_slice(&c),
            Err(_) => open = false,
        }
        while let Ok(c) = rx.try_recv() {
            batch.extend_from_slice(&c);
        }
        if batch.len() >= 256 << 10 || (!open && !batch.is_empty()) {
            m.push(&batch)?;
            batch.clear();
            digest.update(&m.output()[hashed..]);
            hashed = m.output().len();
        }
    }
    if !m.complete() {
        return Err(format!("{name}: download ended mid-member"));
    }
    let data = m.finish()?;
    let sha = hex(digest.finish().as_ref());
    let _slot = SLOTS.take();
    Ok(analyse_specimen_with(name, &data, Some(sha)))
}

/// A member analysed while its archive was still downloading.
struct Streamed {
    name: Vec<u8>,
    csize: u64,
    job: std::thread::JoinHandle<Result<Spec, String>>,
}

/// Walks the local headers of a downloading evidence zip and analyses each `dNN--` member on its own
/// thread (under `SLOTS`) as soon as its bytes are all in, so a time trial's analysis hides under its
/// ~2 s download instead of following it.
struct ZipStream {
    next: usize,
    stopped: bool,
    members: HashMap<u64, Streamed>,
    /// The big member being decoded as it arrives: its data end, bytes forwarded so far, and the channel.
    progressive: Option<(usize, usize, std::sync::mpsc::Sender<Vec<u8>>)>,
    /// `CTF_PROFILE`: when the stream opened, and per member (name, arrived, analysed) in ms from it.
    t0: std::time::Instant,
    log: Option<std::sync::Arc<Mutex<Vec<(String, f64, f64)>>>>,
}

impl Default for ZipStream {
    fn default() -> Self {
        let log = std::env::var_os("CTF_PROFILE").map(|_| Default::default());
        ZipStream { next: 0, stopped: false, members: HashMap::new(), progressive: None, t0: std::time::Instant::now(), log }
    }
}

impl Stream for ZipStream {
    fn feed(&mut self, b: &[u8]) {
        loop {
            if let Some((end, sent, tx)) = &mut self.progressive {
                let upto = b.len().min(*end);
                if upto > *sent {
                    let _ = tx.send(b[*sent..upto].to_vec());
                    *sent = upto;
                }
                if *sent < *end {
                    return;
                }
                self.next = *end;
                self.progressive = None; // closes the channel: the job finishes the member
            }
            if self.stopped {
                return;
            }
            let h = match zip::local_head(b, self.next) {
                Ok(Some(h)) => h,
                Ok(None) => return,
                // The central directory (or something unexpected): `finish` handles the rest.
                Err(_) => {
                    self.stopped = true;
                    return;
                }
            };
            let (at, end) = (self.next, h.end);
            let name = String::from_utf8_lossy(h.raw_name).into_owned();
            let base = name.rsplit('/').next().unwrap_or(&name).to_string();
            let arrived = b.len() >= end;
            if day_of(&base).is_none() {
                if !arrived {
                    return;
                }
                self.next = end;
                continue;
            }
            // A big member (the 67 MB d08 specimen) is decrypted, inflated and hashed while it downloads,
            // leaving only the scans for after its last byte. Its output is reserved against the budget.
            if !arrived && h.usize >= PROGRESSIVE_MIN as u64 {
                let reserve = h.usize as usize;
                if STREAM_BYTES.fetch_add(reserve, Ordering::SeqCst) + reserve <= STREAM_BUDGET {
                    if let Ok(m) = zip::MemberStream::new(&b[at..h.data], b"infected") {
                        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
                        let (t0, log) = (self.t0, self.log.clone());
                        let job = std::thread::spawn(move || {
                            let r = progressive_spec(m, rx, &base);
                            STREAM_BYTES.fetch_sub(reserve, Ordering::SeqCst);
                            if let (Some(log), Ok(s)) = (log, &r) {
                                log.lock().unwrap().push((format!("{} (progressive)", s.name), f64::NAN, t0.elapsed().as_secs_f64() * 1e3));
                            }
                            r
                        });
                        self.members.insert(at as u64, Streamed { name: h.raw_name.to_vec(), csize: h.csize, job });
                        self.progressive = Some((end, h.data, tx));
                        continue;
                    }
                }
                STREAM_BYTES.fetch_sub(reserve, Ordering::SeqCst);
            }
            if !arrived {
                return;
            }
            self.next = end;
            let len = end - at;
            if STREAM_BYTES.fetch_add(len, Ordering::SeqCst) + len > STREAM_BUDGET {
                STREAM_BYTES.fetch_sub(len, Ordering::SeqCst);
                continue;
            }
            let raw = b[at..end].to_vec();
            let (t0, log, arrived) = (self.t0, self.log.clone(), self.t0.elapsed().as_secs_f64() * 1e3);
            let job = std::thread::spawn(move || {
                let r = zip::Zip::local(&raw).and_then(|(z, e)| {
                    let _slot = SLOTS.take();
                    z.read(&e, b"infected").map(|data| analyse_specimen(e.base_name(), &data))
                });
                drop(raw);
                STREAM_BYTES.fetch_sub(len, Ordering::SeqCst);
                if let (Some(log), Ok(s)) = (log, &r) {
                    log.lock().unwrap().push((s.name.clone(), arrived, t0.elapsed().as_secs_f64() * 1e3));
                }
                r
            });
            self.members.insert(at as u64, Streamed { name: h.raw_name.to_vec(), csize: h.csize, job });
        }
    }

    fn finish(self: Box<Self>, d: &Delivery, _: &RunCtx) -> Result<Analysis, String> {
        let landed = self.t0.elapsed().as_secs_f64() * 1e3;
        let r = analyse_delivery(d, self.members);
        if let Some(log) = self.log {
            let mut v = log.lock().unwrap().clone();
            v.sort_by(|a, b| b.2.total_cmp(&a.2));
            for (name, arrived, done) in v.iter().take(4) {
                eprintln!("  [profile] stream {name}: arrived {arrived:.0} ms, analysed {done:.0} ms (body done {landed:.0} ms)");
            }
        }
        r
    }
}

/// A delivery is either the evidence zip itself or an outer zip of per-set evidence zips.
fn analyse_delivery(d: &Delivery, streamed: HashMap<u64, Streamed>) -> Result<Analysis, String> {
    let z = zip::Zip::parse(&d.bytes)?;
    if z.files().any(|e| day_of(e.base_name()).is_some()) {
        return Ok(Box::new(vec![analyse_set(&z, streamed)?]));
    }
    let mut sets = Vec::new();
    for e in z.files().filter(|e| e.name.ends_with(".zip")) {
        let inner = z.read(e, b"infected")?;
        sets.push(analyse_set(&zip::Zip::parse(&inner)?, HashMap::new())?);
    }
    if sets.is_empty() {
        return Err(format!("{}: no dNN specimens", d.name));
    }
    Ok(Box::new(sets))
}

fn container_label(s: &Spec, q: &Question) -> Result<String, String> {
    // Enum (d12+ since Day 19): elf / pe / pe_cli / jar / wasm / neko_bytecode.
    let exact = matches!(q.format_kind.as_deref(), Some("exact_text" | "enum"));
    let syntax = q.format_syntax.as_deref().unwrap_or("");
    Ok(match s.container {
        Some(Container::Pe) if s.is_dotnet && q.format_kind.as_deref() == Some("enum") => "pe_cli".into(),
        Some(Container::Pe) if s.is_dotnet && (syntax.contains("container_runtime") || syntax.contains("managed")) => "PE32_dotnet".into(),
        Some(Container::Elf) => (if exact { "elf" } else { "ELF" }).into(),
        Some(Container::Pe) => (if exact { "pe" } else { "PE" }).into(),
        Some(Container::Jar) => (if exact { "jar" } else { "JAR" }).into(),
        Some(Container::MachO) if !exact => "Mach-O".into(),
        Some(Container::Wasm) => (if exact { "wasm" } else { "WASM" }).into(),
        Some(Container::Neko) if exact => "neko_bytecode".into(),
        other => return Err(format!("no graded label known for container {other:?} (format {:?}: {syntax:?})", q.format_kind)),
    })
}

/// How the HCFG blob is stored (graded d17 single-solve: d08/d09/d11/d13 `base64`, d14 `raw_hcfg_section`,
/// d12 `raw_const_array` = a raw blob in an ordinary data section).
fn hcfg_storage(s: &Spec, h: &hcfg::Hcfg) -> Result<String, String> {
    Ok(match h.source {
        "raw" if s.has_hcfg_section && s.hcfg_section.as_deref().is_some_and(|n| n.trim_start_matches(['.', '_']) == "hcfg") => "raw_hcfg_section".into(),
        "raw" => "raw_const_array".into(),
        "base64" | "jar-constant" => "base64".into(),
        other => return Err(format!("{}: no storage label for HCFG source {other:?} (section {:?})", s.name, s.hcfg_section)),
    })
}

fn config_value(v: &Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_else(|| v.to_json())
}

fn cxx_names(s: &Spec) -> &[String] {
    if s.symbols.is_empty() { &s.mangled_names } else { &s.symbols }
}

/// The `frames` strings of the jar JSON member the question names ("Parse crash/trace.json and ...").
fn trace_frames<'a>(s: &'a Spec, q: &Question) -> Result<Vec<&'a str>, String> {
    let named: Vec<&str> = q.question.split_whitespace().map(|t| t.trim_end_matches(['.', ',', ';', ':'])).filter(|t| s.jar_json.contains_key(*t)).collect();
    let [path] = named.as_slice() else { return Err(format!("{}: question names {named:?} of jar JSON members {:?}", s.name, s.jar_json.keys())) };
    let frames = s.jar_json[*path].get("frames").and_then(Value::as_array).ok_or_else(|| format!("{}: {path} has no frames array", s.name))?;
    frames.iter().map(|f| f.as_str().ok_or_else(|| format!("{}: {path} frame {f:?} is not a string", s.name))).collect()
}

/// The `/regex/` a question quotes ("names matching /event_(success|decoy|failure)/.").
fn question_regex(q: &Question) -> Result<crate::rx::Regex, String> {
    let m = crate::re!(r"(?:^|\s)/(.+?)/(?=[\s.,;:)]|$)").search(&q.question).ok_or_else(|| format!("{}: no /regex/ in the question", q.id))?;
    crate::rx::Regex::new(m.g(1))
}

/// Nonempty SHT_SYMTAB names, in table order.
fn elf_names(s: &Spec) -> Result<&[String], String> {
    if s.container != Some(Container::Elf) {
        return Err(format!("{}: symbol-table question for a non-ELF ({:?})", s.name, s.container));
    }
    Ok(&s.symbols)
}

fn sha256_lines<'a>(mut v: Vec<&'a str>) -> String {
    v.sort_unstable();
    hex(&sha256(v.join("\n").as_bytes()))
}

/// "true exactly when the final bytes [interpreted as Latin-1 or UTF-16LE] contain [the case-sensitive text] X [or Y]
/// [or any nonempty ELF symbol name contains Z]." (Day 28 runtime/decoy predicates). `None`: not this predicate.
fn bytes_predicate(s: &Spec, q: &Question) -> Result<Option<bool>, String> {
    let Some((_, rest)) = q.question.split_once("true exactly when the final bytes ") else { return Ok(None) };
    let rest = rest.split(". ").next().unwrap_or(rest).trim_end().trim_end_matches('.');
    let (on_bytes, on_syms) = match rest.split_once(" or any nonempty ELF symbol name contains ") {
        Some((b, y)) => (b, Some(y)),
        None => (rest, None),
    };
    let (wide, b) = match on_bytes.strip_prefix("interpreted as Latin-1 or UTF-16LE ") {
        Some(b) => (true, b),
        None => (false, on_bytes),
    };
    let b = b.strip_prefix("contain ").ok_or_else(|| format!("{}: unparsed predicate {rest:?}", q.id))?;
    let b = b.strip_prefix("the case-sensitive text ").unwrap_or(b);
    let mut hit = false;
    for n in b.split(" or ") {
        let (latin1, utf16) = s.needles.get(n).ok_or_else(|| format!("{}: predicate text {n:?} is not one of the scanned NEEDLES", s.name))?;
        hit |= *latin1 || (wide && *utf16);
    }
    for n in on_syms.into_iter().flat_map(|y| y.split(" or ")) {
        hit |= elf_names(s)?.iter().chain(&s.dynsym_names).any(|x| x.contains(n));
    }
    Ok(Some(hit))
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
        "analysis_machine" if q.format_kind.as_deref() == Some("integer") => need(s.machine_num.map(|n| n.to_string()), "machine number")?,
        // A PE i386 image under the generic "file, readelf, and objdump labels" syntax is graded `i386` (d22; `x86` and
        // `0x14c` were incorrect there). d03's PE-specific "IMAGE_FILE_MACHINE labels" syntax keeps `x86`.
        "analysis_machine"
            if s.container == Some(Container::Pe)
                && s.machine_num == Some(0x14c)
                && !q.format_syntax.as_deref().unwrap_or("").contains("IMAGE_FILE_MACHINE") =>
        {
            "i386".into()
        }
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
        // A stripped build has no recover_code symbol: the question then turns exact_text ("none" when absent).
        // Since Day 19 the question is a `pattern` whose syntax names the `none` token.
        "analysis_recover_code"
            if matches!(s.disassembly, Some(Ok(ref v)) if v.is_empty())
                && (q.format_kind.as_deref() == Some("exact_text") || q.format_syntax.as_deref().is_some_and(|x| x.contains("literal token none"))) =>
        {
            "none".into()
        }
        "analysis_recover_code" => need(s.recover_code.map(|v| v.to_string()), "recover_code return value")?,
        "analysis_typeinfo_symbol" => need(typeinfo_symbol(cxx_names(s)), "unambiguous _ZTI typeinfo symbol")?,
        f if f.starts_with("analysis_mangled_") && f.ends_with("_symbol") => {
            let ident = &f["analysis_mangled_".len()..f.len() - "_symbol".len()];
            need(mangled_symbol(cxx_names(s), ident), &format!("mangled symbol for {ident:?}"))?
        }
        "analysis_has_go_runtime" => s.go_runtime.to_string(),
        "analysis_has_rust_mangled_symbol" => s.rust_mangled.to_string(),
        "analysis_has_coroutine_runtime" if s.container == Some(Container::Jar) => s.coroutine_runtime.to_string(),
        "analysis_methods" => {
            let c = s.main_class.as_ref().ok_or_else(|| format!("{}: no Main-Class", s.name))?;
            json(&c.methods.iter().map(|m| m.0.clone()).collect::<Vec<_>>())
        }
        "analysis_method_count" => need(s.main_class.as_ref().map(|c| c.methods.len().to_string()), "Main-Class")?,
        "analysis_access_flags" => need(s.main_class.as_ref().map(|c| c.access.to_string()), "Main-Class")?,
        "analysis_crash_fault_method" => trace_frames(s, q)?.first().ok_or_else(|| format!("{}: empty frames", s.name))?.to_string(),
        // "true exactly when every string in crash/trace.json frames is present in the primary classfile method-name set".
        "analysis_crash_frames_match_methods" => {
            let c = s.main_class.as_ref().ok_or_else(|| format!("{}: no Main-Class", s.name))?;
            trace_frames(s, q)?.iter().all(|f| c.methods.iter().any(|m| m.0 == *f)).to_string()
        }
        "analysis_hcfg_storage" => hcfg_storage(s, h()?)?,
        "analysis_section_count" => need(s.section_count.map(|n| n.to_string()), "ELF section headers")?,
        "analysis_conditional_branch_opcode_count" => need(s.cond_branch_opcodes.map(|n| n.to_string()), "ELF .text section")?,
        "analysis_class_count" => need(s.class_count.map(|n| n.to_string()), "jar central directory")?,
        "analysis_class_name" => need(s.main_class.as_ref().map(|c| c.this_class.clone()), "Main-Class")?,
        "analysis_method_names_sha256" => {
            let c = s.main_class.as_ref().ok_or_else(|| format!("{}: no Main-Class", s.name))?;
            let mut names: Vec<&str> = c.methods.iter().map(|m| m.0.as_str()).collect();
            names.sort_unstable();
            names.dedup();
            sha256_lines(names)
        }
        "analysis_cli_runtime" => need(s.cli_runtime.map(|(a, b)| format!("{a}.{b}")), "CLI header")?,
        f if f.starts_with("analysis_packing_key_") && f.ends_with("_hex") => {
            let (k1, k2) = s.packing_keys.ok_or_else(|| format!("{}: no validated PK19 overlay", s.name))?;
            match &f["analysis_packing_key_".len()..f.len() - "_hex".len()] {
                "1" => format!("0x{k1:02x}"),
                "2" => format!("0x{k2:02x}"),
                n => return Err(format!("{}: no packing key {n:?}", s.name)),
            }
        }
        "analysis_symbol_count" => elf_names(s)?.len().to_string(),
        f if f.ends_with("_symbol_count") => {
            let r = question_regex(q)?;
            elf_names(s)?.iter().filter(|n| r.is_match(n)).count().to_string()
        }
        f if f.ends_with("_symbols_sha256") => {
            let r = question_regex(q)?;
            sha256_lines(elf_names(s)?.iter().map(String::as_str).filter(|n| r.is_match(n)).collect())
        }
        // ELF: SHT_SYMTAB entries; PE: primary COFF symbols (the d02 wording spells out both traversals).
        f if f.ends_with("_symbol") && q.question.contains("first exact remaining name matching") => {
            let r = question_regex(q)?;
            let names = if s.container == Some(Container::Pe) { &s.coff_symbols[..] } else { elf_names(s)? };
            need(names.iter().find(|n| r.is_match(n)).cloned(), "symbol matching the question's regex")?
        }
        "analysis_overlay_offset" => need(s.overlay.map(|o| o.0.to_string()), "validated packed overlay")?,
        "analysis_overlay_size" => need(s.overlay.map(|o| o.1.to_string()), "validated packed overlay")?,
        "analysis_hcfg_resource_size" => need(s.hcfg_resource_size.map(|n| n.to_string()), "validated resource overlay")?,
        f if s.objects.contains_key(&f["analysis_".len().min(f.len())..]) => s.objects[&f["analysis_".len()..]].clone(),
        f if f.starts_with("analysis_has_") && bytes_predicate(s, q)?.is_some() => bytes_predicate(s, q)?.unwrap_or_default().to_string(),
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
        analyse_delivery(d, HashMap::new())
    }

    fn stream(&self, _: &RunCtx) -> Option<Box<dyn Stream>> {
        Some(Box::new(ZipStream::default()))
    }

    fn answer(&self, q: &Question, done: &Analysed, ctx: &RunCtx) -> Result<String, String> {
        enum_checked(q, answer_any(q, done, ctx)?)
    }
}

/// An `enum` answer must be one of `accepted_values` (a unique case-insensitive match is taken verbatim).
fn enum_checked(q: &Question, a: String) -> Result<String, String> {
    let Some(vals) = q.raw.get("answer_format").and_then(|f| f.get("accepted_values")).and_then(Value::as_array) else { return Ok(a) };
    let vals: Vec<&str> = vals.iter().filter_map(Value::as_str).collect();
    if vals.contains(&a.as_str()) {
        return Ok(a);
    }
    match vals.iter().filter(|v| v.eq_ignore_ascii_case(&a)).collect::<Vec<_>>().as_slice() {
        [one] => Ok(one.to_string()),
        _ => Err(format!("{}: {a:?} is not one of {vals:?}", q.id)),
    }
}

/// A role tag that is not in the member name (d29: `analysis_optimized_*` for "basename ends O3.elf"; d30:
/// `config_capsule_format` for `-config.n`): drop leading words of the field until it is a known one.
fn answer_dropping(rest: &str, s: &Spec, q: &Question) -> Result<String, String> {
    let words: Vec<&str> = rest.split('_').collect();
    let mut r = answer_spec(&format!("analysis_{rest}"), s, &[], q);
    for k in 1..words.len() {
        match &r {
            Err(e) if e.starts_with("unknown field") => r = answer_spec(&format!("analysis_{}", words[k..].join("_")), s, &[], q),
            _ => break,
        }
    }
    r
}

/// A JSON path segment whose keys may themselves be joined by `_` (`trace_branch` -> `trace.branch`, Day 31).
fn lookup<'a>(v: &'a Value, seg: &str) -> Option<&'a Value> {
    if let Some(x) = v.get(seg) {
        return Some(x);
    }
    seg.match_indices('_').find_map(|(i, _)| lookup(v.get(&seg[..i])?, &seg[i + 1..]))
}

/// Questions on a record with the HCFG layout under an `HG??` magic (d30 `HGRF`, d31 `HGAU`): "Locate the unique
/// CRC32-valid HGAU record ..." plus a JSON path ("at loader.n decoded HGRF JSON property graph.config.url",
/// "the exact gauntlet.trace_branch token") or a header field (`*_crc32_hex`, `*_xor_key_hex`). The path's
/// leading root name (`graph`, `gauntlet`) is dropped when the JSON has no such key. `None`: not such a question.
fn record_answer(q: &Question, field: &str, day: &str, spec: &Spec, others: &[Spec]) -> Option<Result<String, String>> {
    let magic = crate::re!(r"\b(HG[A-Z0-9]{2})\b").search(&q.question)?.g(1).to_string();
    let prop = crate::re!(r"at (?:(\S+) )?decoded HG[A-Z0-9]{2} JSON property ([A-Za-z0-9_.]+?)[,.]?(?:\s|$)").search(&q.question);
    let path = prop.as_ref().map(|m| m.g(2)).or_else(|| crate::re!(r"exact ([A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)+) token").search(&q.question).map(|m| m.g(1)));
    let header = ["crc32_hex", "xor_key_hex"].into_iter().find(|h| field.ends_with(h));
    if path.is_none() && header.is_none() {
        return None;
    }
    Some((|| {
        let members: Vec<&Spec> = std::iter::once(spec).chain(others).collect();
        let named = prop.as_ref().and_then(|m| m.group(1)).map(str::to_string).or_else(|| named_member(&q.question, day));
        let pick: Vec<&Spec> = match &named {
            Some(n) => members.iter().copied().filter(|s| member_matches(&s.name, n)).collect(),
            None => members.iter().copied().filter(|s| s.records.contains_key(&magic)).collect(),
        };
        let [s] = pick.as_slice() else { return Err(format!("{day}: {} members for the {magic} record (named {named:?})", pick.len())) };
        let g = s.records.get(&magic).ok_or_else(|| format!("{}: no CRC-valid {magic} record", s.name))?;
        // "after confirming all three canonical campaign graphs match when this_role is omitted" (d30).
        if q.question.contains("graphs match when this_role is omitted") {
            let canon = |s: &Spec| {
                s.records.get(&magic).map(|g| match &g.config {
                    Value::Obj(kv) => Value::Obj(kv.iter().filter(|(k, _)| k != "this_role").cloned().collect()).to_json(),
                    v => v.to_json(),
                })
            };
            let all: Vec<Option<String>> = members.iter().filter(|s| !s.name.ends_with(".json")).map(|s| canon(s)).collect();
            if all.iter().any(|c| c.is_none() || *c != all[0]) {
                return Err(format!("{day}: {magic} records differ (this_role omitted) or are missing"));
            }
        }
        let Some(path) = path else {
            return Ok(match header {
                Some("crc32_hex") => format!("0x{:08x}", g.crc),
                _ => format!("0x{:02x}", g.key),
            });
        };
        let mut segs: Vec<&str> = path.split('.').collect();
        if segs.len() > 1 && lookup(&g.config, segs[0]).is_none() {
            segs.remove(0);
        }
        let mut v = &g.config;
        for k in &segs {
            v = lookup(v, k).ok_or_else(|| format!("{}: {magic} JSON has no {path:?}", s.name))?;
        }
        Ok(config_value(v))
    })())
}

fn answer_any(q: &Question, done: &Analysed, _: &RunCtx) -> Result<String, String> {
    {
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
        let (spec, side, others) = set.days.get(day).ok_or_else(|| format!("no {day} specimen in the evidence"))?;
        // Whole-member-set fields (Day 28): every dNN-- member in central-directory order.
        if let Some(f) = field.strip_prefix("structural_") {
            let m = set.members.get(day).ok_or_else(|| format!("no {day} members"))?;
            return match f {
                "artifact_count" => Ok(m.len().to_string()),
                "ordered_set_sha256" => Ok(hex(&sha256(m.iter().map(|(_, h)| h.as_str()).collect::<Vec<_>>().join(":").as_bytes()))),
                other => Err(format!("unknown structural field {other:?} for {day}")),
            };
        }
        if let Some(r) = record_answer(q, field, day, spec, others) {
            return r;
        }
        // `analysis_<variant>_<field>` addresses the specimen whose name carries `-<variant>`
        // (d15: `-stripped` / `-symbolized`; d28: `-false-friend` -> `false_friend_`). The longest tag wins.
        if let Some(rest) = field.strip_prefix("analysis_") {
            let mut best: Option<(usize, &Spec, &str)> = None;
            for s in std::iter::once(spec).chain(others) {
                let stem = s.name.rsplit_once('.').map_or(s.name.as_str(), |(a, _)| a);
                let Some((_, label)) = stem.split_once("--") else { continue };
                let parts: Vec<&str> = label.split('-').collect();
                for k in 1..=parts.len() {
                    let tag = parts[parts.len() - k..].join("_");
                    if !tag.bytes().any(|b| b.is_ascii_alphabetic()) || best.is_some_and(|(n, _, _)| n >= tag.len()) {
                        continue;
                    }
                    if let Some(f) = rest.strip_prefix(tag.as_str()).and_then(|f| f.strip_prefix('_')) {
                        best = Some((tag.len(), s, f));
                    }
                }
            }
            if let Some((_, s, f)) = best {
                return answer_dropping(f, s, q);
            }
            // "Decode X and Y and require their config objects to be byte-for-byte equal before using the common
            // value; do not use Z" (d27: "both d27-- role members"): every named (else every) member must decode
            // the same config.
            if rest.starts_with("config_") && q.question.contains("byte-for-byte equal") {
                let all: Vec<&Spec> = std::iter::once(spec).chain(others).filter(|s| !s.name.ends_with(".json")).collect();
                let named = |text: &str| -> Vec<&Spec> {
                    text.split_whitespace()
                        .map(|t| t.trim_end_matches(['.', ',', ';', ':']))
                        .filter(|t| t.len() >= 3 && t.contains(['.', '-']))
                        .filter_map(|t| match all.iter().filter(|s| member_matches(&s.name, t)).collect::<Vec<_>>().as_slice() {
                            [one] => Some(**one),
                            _ => None,
                        })
                        .collect()
                };
                let head = q.question.split(" require ").next().unwrap_or("");
                let skip = q.question.split_once("do not use ").map_or(Vec::new(), |(_, r)| named(r.split(". ").next().unwrap_or(r)));
                let mut pick = named(head);
                if pick.is_empty() {
                    pick = all.clone();
                }
                pick.retain(|s| !skip.iter().any(|x| x.name == s.name));
                let cfgs: Vec<(u32, String)> = pick
                    .iter()
                    .map(|s| s.hcfg.as_ref().map(|h| (h.crc, h.config.to_json())).ok_or_else(|| format!("{}: no HCFG blob found", s.name)))
                    .collect::<Result<_, _>>()?;
                if cfgs.is_empty() || cfgs.iter().any(|c| *c != cfgs[0]) {
                    let names: Vec<&str> = pick.iter().map(|s| s.name.as_str()).collect();
                    return Err(format!("{day}: configs of {names:?} are not equal (or none named)"));
                }
                return answer_spec(field, pick[0], &[], q);
            }
            // An untagged field whose text names another member (d21: `config_*` "Use the d21-- plugin.cmxs member;
            // the loader has no scored HCFG config") is answered from that member.
            if let Some(s) = named_member(&q.question, day).and_then(|m| {
                let hits: Vec<&Spec> = std::iter::once(spec).chain(others).filter(|s| member_matches(&s.name, &m)).collect();
                (hits.len() == 1).then(|| hits[0])
            }) {
                return answer_dropping(rest, s, q);
            }
        }
        answer_spec(field, spec, side, q)
    }
}

#[cfg(test)]
mod tests {
    use super::SLOTS;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn slots_cap_concurrent_members() {
        let limit = std::thread::available_parallelism().map_or(2, |n| n.get()).max(1);
        let (live, peak) = (AtomicUsize::new(0), AtomicUsize::new(0));
        std::thread::scope(|s| {
            for _ in 0..limit * 3 {
                s.spawn(|| {
                    let _slot = SLOTS.take();
                    let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    live.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        assert!(peak.load(Ordering::SeqCst) <= limit);
        assert_eq!(*SLOTS.busy.lock().unwrap(), 0);
    }
}
