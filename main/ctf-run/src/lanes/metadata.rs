//! Metadata forensics: one question kind per day (file hashes, image geometry, PDF/ZIP/PE/.NET/MSI
//! fields, ...); time-trial and mastery sometimes ask the whole cumulative bundle. Kinds are matched
//! on the question-id suffix (after `setN.`), and answer formats are the graded ones from the Python
//! reference (`0x%04x` machine, `0x%08x` entry RVA, lowercase MVID, `{UPPER}` MSI GUIDs, compact JSON).
//! A value the format defines as absent is the literal `none`. Unknown kinds fail with a FACTS dump.

use super::{Analysed, Analysis, Lane};
use crate::crypto::{md5::md5, sha1::sha1, sha256::sha256};
use crate::evidence::Registry;
use crate::formats::bytes::{find, hex, u32le};
use crate::formats::{cfb, dotnet, elf, lnk, media, ooxml, pe, zip};
use crate::json::Value;
use crate::model::{Delivery, Question, RunCtx};

pub struct Metadata;

fn none() -> String {
    "none".into()
}

/// Raw EOCD fields of the last valid record: (total entries, cd size, cd offset, comment length).
fn eocd(d: &[u8]) -> Option<(u16, u32, u32, u16)> {
    let i = zip::find_eocd(d)?;
    Some((u16::from_le_bytes([d[i + 10], d[i + 11]]), u32le(d, i + 12)?, u32le(d, i + 16)?, u16::from_le_bytes([d[i + 20], d[i + 21]])))
}

/// Central-directory names, raw and in stored order (UTF-8 when flagged, else CP437). Walks from the
/// EOCD's offset; falls back to scanning for directory signatures.
fn cd_names(d: &[u8]) -> Vec<String> {
    let name_at = |p: usize| -> Option<(String, usize)> {
        let flag = u16::from_le_bytes([*d.get(p + 8)?, *d.get(p + 9)?]);
        let n = u16::from_le_bytes([*d.get(p + 28)?, *d.get(p + 29)?]) as usize;
        let x = u16::from_le_bytes([*d.get(p + 30)?, *d.get(p + 31)?]) as usize;
        let c = u16::from_le_bytes([*d.get(p + 32)?, *d.get(p + 33)?]) as usize;
        let raw = d.get(p + 46..p + 46 + n)?;
        let s = if flag & 0x800 != 0 { String::from_utf8_lossy(raw).into_owned() } else { crate::formats::bytes::cp437(raw) };
        Some((s, 46 + n + x + c))
    };
    let mut out = Vec::new();
    if let Some((_, _, off, _)) = eocd(d) {
        let mut p = off as usize;
        if d.get(p..p + 4) == Some(b"PK\x01\x02") {
            while d.get(p..p + 4) == Some(b"PK\x01\x02") {
                let Some((s, len)) = name_at(p) else { break };
                out.push(s);
                p += len;
            }
            return out;
        }
    }
    for p in crate::formats::bytes::find_all(d, b"PK\x01\x02") {
        if let Some((s, _)) = name_at(p) {
            out.push(s);
        }
    }
    out
}

fn compact_json(items: &[String]) -> String {
    Value::Arr(items.iter().map(|s| Value::Str(s.clone())).collect()).to_json()
}

fn pe_or_none(d: &[u8], f: impl Fn(&pe::Pe) -> String) -> String {
    pe::Pe::parse(d).map(|p| f(&p)).unwrap_or_else(none)
}

fn msi(d: &[u8], key: &str) -> Option<String> {
    cfb::msi_properties(d)?.get(key).cloned()
}

type Handler = fn(&[u8]) -> Option<String>;

/// (id suffix, handler), most specific first.
const HANDLERS: &[(&str, Handler)] = &[
    ("-size-bytes", |b| Some(b.len().to_string())),
    ("-md5", |b| Some(hex(&md5(b)))),
    ("-sha1", |b| Some(hex(&sha1(b)))),
    ("-sha256", |b| Some(hex(&sha256(b)))),
    ("magic-prefix-hex-16", |b| Some(hex(&b[..b.len().min(16)]))),
    ("utf8-bom-present", |b| Some(b.starts_with(&[0xEF, 0xBB, 0xBF]).to_string())),
    ("jpeg-sof-dimensions-pixels", |b| Some(media::jpeg_sof(b).map(|(w, h)| format!("{w}x{h}")).unwrap_or_else(none))),
    ("png-ihdr-dimensions-pixels", |b| Some(media::png_ihdr(b).map(|(w, h)| format!("{w}x{h}")).unwrap_or_else(none))),
    ("pdf-header-version", |b| Some(media::pdf_header_version(b).unwrap_or_else(none))),
    ("zip-central-directory-member-names", |b| Some(compact_json(&cd_names(b)))),
    ("eocd-total-entries", |b| Some(eocd(b).map(|e| e.0.to_string()).unwrap_or_else(none))),
    ("pe-coff-machine-u16-hex", |b| Some(pe_or_none(b, |p| format!("0x{:04x}", p.machine)))),
    ("pe-address-of-entry-point-rva-hex", |b| Some(pe_or_none(b, |p| format!("0x{:08x}", p.entry_rva)))),
    ("pe-version-original-filename", |b| Some(pe::version_string(b, "OriginalFilename").unwrap_or_else(none))),
    ("elf-machine-u16-hex", |b| Some(elf::Elf::parse(b).map(|e| format!("0x{:04x}", e.machine)).unwrap_or_else(none))),
    ("dotnet-mvid-guid", |b| Some(dotnet::Metadata::find(b).and_then(|m| m.mvid()).unwrap_or_else(none))),
    // MSI GUIDs are graded as stored: uppercase with braces.
    ("msi-product-code-guid", |b| msi(b, "ProductCode")),
    ("msi-upgrade-code-guid", |b| msi(b, "UpgradeCode")),
    ("msi-product-version", |b| msi(b, "ProductVersion")),
    ("msi-product-name", |b| msi(b, "ProductName")),
    ("msi-manufacturer", |b| msi(b, "Manufacturer")),
    ("msi-package-code-guid", |b| cfb::summary_string(b, 9)),
    ("msi-revision-number", |b| cfb::summary_string(b, 9)),
];

/// `...-ooxml-<field>` -> a package field: relationship lists (compact JSON, **sorted by value**: the
/// d15 single-solve graded `[control, evidence/..]` against file order rId9, rId10 - a value sort and an
/// rId string sort agree there) or a core/app property.
fn ooxml_answer(key: &str, b: &[u8]) -> Option<Result<String, String>> {
    let i = key.rfind("ooxml-")?;
    let want = key[i + 6..].replace('-', "_");
    let Some(p) = ooxml::Package::parse(b) else { return Some(Err("not an OOXML package".into())) };
    let rels = p.relationships();
    let list = |f: &dyn Fn(&ooxml::Rel) -> bool, g: &dyn Fn(&ooxml::Rel) -> String| {
        let mut v: Vec<String> = rels.iter().filter(|r| f(r)).map(g).collect();
        v.sort();
        compact_json(&v)
    };
    let ext = |r: &ooxml::Rel| r.external;
    Some(Ok(match want.as_str() {
        "external_relationship_targets" | "external_targets" | "external_relationship_target_list" => list(&ext, &|r| r.target.clone()),
        "external_relationship_ids" => list(&ext, &|r| r.id.clone()),
        "external_relationship_count" => rels.iter().filter(|r| r.external).count().to_string(),
        "relationship_count" => rels.len().to_string(),
        "hyperlink_targets" => list(&|r| r.kind.ends_with("/hyperlink"), &|r| r.target.clone()),
        other => {
            let props: Vec<(String, String)> = p.core_properties().into_iter().chain(p.app_properties()).collect();
            let flat = |s: &str| s.replace('_', "").to_ascii_lowercase();
            let name = other.strip_prefix("core_").or_else(|| other.strip_prefix("app_")).unwrap_or(other);
            match props.iter().find(|(k, _)| flat(k) == flat(name)) {
                Some((_, v)) => v.clone(),
                None => return Some(Err(format!("unknown OOXML field {other:?}; {}", ooxml_facts(&p)))),
            }
        }
    }))
}

fn ooxml_facts(p: &ooxml::Package) -> String {
    let rels: Vec<String> = p.relationships().iter().map(|r| format!("{}#{} {} -> {}{}", r.part, r.id, r.kind.rsplit('/').next().unwrap_or(""), r.target, if r.external { " (External)" } else { "" })).collect();
    format!("ooxml parts {:?} rels {rels:?} core {:?} app {:?}", p.parts.iter().map(|x| &x.0).collect::<Vec<_>>(), p.core_properties(), p.app_properties())
}

/// `...-lnk-<field>` -> the parsed shell-link field (graded formats unknown until first seen).
fn lnk_answer(key: &str, b: &[u8]) -> Option<Result<String, String>> {
    let i = key.rfind("lnk-")?;
    let want = key[i + 4..].replace('-', "_");
    let field = match want.as_str() {
        "target" | "target_path" | "target_path_windows" | "link_target" | "target_path_string" | "resolved_target" => {
            return Some(lnk::parse(b).ok_or_else(|| "not a shell link".to_string()).and_then(|f| lnk_target(&f)));
        }
        "working_directory" | "working_dir" => "working_dir",
        "arguments" | "command_line_arguments" | "command_line" => "arguments",
        "description" | "comment" | "name" | "name_string" => "name",
        "env_target" | "environment_target" | "environment_variable_target" => "environment_target",
        "tracker_machine_id" | "machine_id" | "netbios_name" => "machine_id",
        "volume_serial" | "drive_serial" | "volume_serial_number" => "drive_serial",
        other => {
            let f = lnk::parse(b)?;
            return Some(f.get(other).cloned().ok_or_else(|| format!("unknown LNK field {other:?}; fields: {f:?}")));
        }
    };
    let f = lnk::parse(b)?;
    Some(f.get(field).cloned().ok_or_else(|| format!("LNK has no {field}; fields: {f:?}")))
}

/// The link's absolute Windows target. Every source the file carries - LinkInfo (base/share + suffix),
/// the IDList shell-item path, and a literal environment-block target - must agree (case-insensitively);
/// LinkInfo's spelling is reported. Disagreement or a relative-only link is an error, never a pick.
fn lnk_target(f: &std::collections::BTreeMap<&'static str, String>) -> Result<String, String> {
    let env = f.get("environment_target").filter(|s| !s.contains('%'));
    let sources: Vec<(&str, &String)> = [("linkinfo", f.get("target_path")), ("idlist", f.get("idlist_path")), ("environment", env)]
        .into_iter()
        .filter_map(|(n, v)| v.map(|v| (n, v)))
        .collect();
    let Some(&(_, first)) = sources.first() else {
        if f.contains_key("relative_path") || f.contains_key("environment_target") {
            return Err(format!("LNK target is relative or variable-only; fields: {f:?}"));
        }
        return Ok(none());
    };
    if sources.iter().any(|(_, v)| !v.eq_ignore_ascii_case(first)) {
        return Err(format!("LNK target sources disagree {sources:?}; fields: {f:?}"));
    }
    Ok(first.clone())
}

/// Everything the parsers can see in one file (for errors on unknown kinds).
pub fn facts(name: &str, b: &[u8]) -> String {
    let mut f = vec![format!("{name}: {} B, magic {}", b.len(), hex(&b[..b.len().min(16)]))];
    if let Some((w, h)) = media::jpeg_sof(b) {
        f.push(format!("jpeg SOF {w}x{h}"));
    }
    if let Some((chunks, trailing)) = media::png_chunks(b) {
        let kinds: Vec<String> = chunks.iter().map(|c| format!("{}{}", c.kind, if c.crc_ok { "" } else { "(badcrc)" })).collect();
        f.push(format!("png IHDR {:?} chunks {kinds:?} trailing {trailing}", media::png_ihdr(b)));
    }
    if let Some(v) = media::pdf_header_version(b) {
        let (versions, eofs) = media::pdf_facts(b);
        f.push(format!("pdf header {v} /Version {versions:?} %%EOF x{eofs}"));
    }
    if let Some(p) = ooxml::Package::parse(b) {
        f.push(ooxml_facts(&p));
    }
    if let Some(e) = eocd(b) {
        f.push(format!("zip EOCD total {} cd_size {} cd_off {} comment {}; names {:?}", e.0, e.1, e.2, e.3, cd_names(b)));
    }
    if let Some(p) = pe::Pe::parse(b) {
        f.push(format!(
            "pe machine 0x{:04x} entry 0x{:08x} sections {:?} OriginalFilename {:?} dotnet {}",
            p.machine,
            p.entry_rva,
            p.sections.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            pe::version_string(b, "OriginalFilename"),
            p.is_dotnet()
        ));
    }
    if let Some(m) = dotnet::Metadata::find(b) {
        f.push(format!("dotnet {} mvid {:?}", m.version, m.mvid()));
    }
    if let Some(e) = elf::Elf::parse(b) {
        f.push(format!("elf class {} machine {} type {} entry 0x{:x}", e.class, e.machine, e.kind, e.entry));
    }
    if let Some(props) = cfb::msi_properties(b) {
        f.push(format!("msi {props:?} revision {:?}", cfb::summary_string(b, 9)));
    }
    if let Some(l) = lnk::parse(b) {
        f.push(format!("lnk {l:?}"));
    }
    if find(b, &[0xEF, 0xBB, 0xBF]) == Some(0) {
        f.push("utf8 BOM".into());
    }
    f.join("; ")
}

/// `<stem>-filename` -> the member whose name minus extension equals the stem.
fn filename_answer(members: &[(String, Vec<u8>)], key: &str) -> Option<String> {
    let stem = key.strip_suffix("-filename")?;
    members
        .iter()
        .find(|(n, _)| n.rsplit_once('.').map(|(s, _)| s).unwrap_or(n).to_lowercase() == stem)
        .or_else(|| members.iter().find(|(n, _)| n.to_lowercase().starts_with(stem)))
        .map(|(n, _)| n.clone())
}

impl Lane for Metadata {
    fn name(&self) -> &'static str {
        "metadata"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &["metadata-forensics-day-"]
    }

    fn analyse(&self, d: &Delivery, _: &RunCtx) -> Result<Analysis, String> {
        Ok(Box::new(Registry::load(&d.name, &d.bytes)?))
    }

    fn answer(&self, q: &Question, done: &Analysed, _: &RunCtx) -> Result<String, String> {
        let key = q.key.replace('_', "-");
        let label = q.artifact.as_deref().unwrap_or("");
        // The registry holding the named zip; else the question's delivery; else the only one.
        let regs: Vec<&Registry> = done.all::<Registry>().map(|(_, r)| r).collect();
        let (zname, _) = crate::evidence::parse_label(label);
        // An explicit `delivery N:` wins (mastery deliveries can share a file name), then the zip name.
        let reg: &Registry = if let Some((n, _)) = q.delivery_label() {
            done.get::<Registry>(n.checked_sub(1).ok_or("delivery 0")?)?
        } else if let Some(r) = regs.iter().find(|r| r.has_zip(zname)) {
            r
        } else if regs.len() == 1 {
            regs[0]
        } else {
            return Err(format!("no evidence for {label:?}"));
        };
        let (members, member, data) = reg.resolve(label);

        if key.contains("synthesis-sha256-order") {
            let text = &q.question;
            let i = text.find("Files: [").ok_or("synthesis question without a Files: list")?;
            let end = text[i..].find(']').ok_or("unterminated Files list")? + i + 1;
            let names: Vec<String> = Value::parse(&text[i + 7..end])
                .map_err(|e| e.to_string())?
                .as_array()
                .ok_or("Files is not a list")?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            let mut keyed = Vec::new();
            for n in names {
                let b = members.iter().find(|(m, _)| *m == n).ok_or_else(|| format!("synthesis file {n:?} missing"))?;
                keyed.push((hex(&sha256(&b.1)), n));
            }
            keyed.sort();
            return Ok(compact_json(&keyed.into_iter().map(|(_, n)| n).collect::<Vec<_>>()));
        }
        if key.ends_with("-filename") && !key.ends_with("original-filename") {
            if !member.is_empty() {
                return Ok(member.to_string());
            }
            return filename_answer(members, &key).ok_or_else(|| format!("no member matches {key:?} in {:?}", members.iter().map(|m| &m.0).collect::<Vec<_>>()));
        }
        let data = data.ok_or_else(|| format!("no file {member:?} for {label:?} (members {:?})", members.iter().map(|m| &m.0).collect::<Vec<_>>()))?;
        for (suffix, h) in HANDLERS {
            if key.ends_with(suffix) {
                return h(data).ok_or_else(|| format!("{suffix}: not derivable; {}", facts(member, data)));
            }
        }
        if let Some(r) = lnk_answer(&key, data) {
            return r;
        }
        if let Some(r) = ooxml_answer(&key, data) {
            return r;
        }
        Err(format!("unknown kind {key:?}; {}", facts(member, data)))
    }
}
