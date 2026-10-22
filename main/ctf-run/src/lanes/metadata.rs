//! Metadata forensics: one question kind per day (file hashes, image geometry, PDF/ZIP/PE/.NET/MSI
//! fields, ...); time-trial and mastery sometimes ask the whole cumulative bundle. Kinds are matched
//! on the question-id suffix (after `setN.`), and answer formats are the graded ones from the Python
//! reference (`0x%04x` machine, `0x%08x` entry RVA, lowercase MVID, `{UPPER}` MSI GUIDs, compact JSON).
//! A value the format defines as absent is the literal `none`. Unknown kinds fail with a FACTS dump.

use super::{Analysed, Analysis, Lane};
use crate::crypto::{md5::md5, sha1::sha1, sha256::sha256};
use crate::evidence::Registry;
use crate::formats::bytes::{find, hex, u32le};
use crate::formats::{apk, audio, cfb, dotnet, elf, email, jar, lnk, macho, media, ooxml, pe, video, zip};
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

/// `...-email-<field>` -> a message field. Headers are answered by name (`email-message-id` ->
/// `Message-ID`, unfolded, RFC 2047 decoded for free text); msg-id lists are space-joined tokens.
/// Attachment/part fields are derived from the decoded MIME leaves.
fn email_answer(key: &str, b: &[u8]) -> Option<Result<String, String>> {
    let i = key.rfind("email-")?;
    let want = &key[i + 6..];
    let Some(m) = email::Message::parse(b) else { return Some(Err(format!("not an email message; {}", facts("eml", b)))) };
    let atts = m.attachments();
    let ids = |h: &str| m.header(h).map(|v| email::msg_ids(v).join(" "));
    let r = match want {
        "message-id" | "in-reply-to" => ids(want).filter(|s| !s.is_empty()).or_else(|| m.header(want).map(str::to_string)).unwrap_or_else(none),
        "references" => ids(want).filter(|s| !s.is_empty()).unwrap_or_else(none),
        "attachment-count" => atts.len().to_string(),
        "part-count" | "mime-part-count" => m.parts().len().to_string(),
        "attachment-filename" | "attachment-name" => match atts.as_slice() {
            [] => none(),
            [a] => a.filename().unwrap_or_else(none),
            _ => return Some(Err(format!("several attachments; {}", email_facts(&m)))),
        },
        "attachment-filenames" | "attachment-names" => compact_json(&atts.iter().filter_map(|a| a.filename()).collect::<Vec<_>>()),
        "attachment-sha256" | "attachment-md5" | "attachment-sha1" | "attachment-size-bytes" => match atts.as_slice() {
            [] => none(),
            [a] => match want.rsplit('-').next().unwrap() {
                "sha256" => hex(&sha256(&a.data)),
                "md5" => hex(&md5(&a.data)),
                "sha1" => hex(&sha1(&a.data)),
                _ => a.data.len().to_string(),
            },
            _ => return Some(Err(format!("several attachments; {}", email_facts(&m)))),
        },
        other => match m.header(other) {
            Some(v) => email::decode_words(v),
            None => return Some(Err(format!("unknown email field {other:?}; {}", email_facts(&m)))),
        },
    };
    Some(Ok(r))
}

fn email_facts(m: &email::Message) -> String {
    let parts: Vec<String> = m.parts().iter().map(|p| format!("{} {:?} {} B sha256 {}", p.content_type(), p.filename(), p.data.len(), hex(&sha256(&p.data)))).collect();
    format!("email headers {:?} parts {parts:?}", m.headers)
}

/// `...-media-<field>` -> an audio stream field, sniffed from the bytes (the extension may lie). Codec
/// names are ffprobe `codec_name`s (d17 graded `pcm_s16le`); "primary" = the first audio stream. A
/// mail message stands for its single audio attachment. Fields other than the codec are unverified.
fn media_answer(key: &str, b: &[u8]) -> Option<Result<String, String>> {
    let i = key.rfind("media-")?;
    let want = key[i + 6..].strip_prefix("primary-").unwrap_or(&key[i + 6..]);
    if let Some(field) = want.strip_prefix("video-") {
        return Some(video_answer(field, b));
    }
    let want = want.strip_prefix("audio-").unwrap_or(want);
    let owned;
    let data = match (audio::sniff(b), email::Message::parse(b)) {
        (Some(_), _) => b,
        (None, Some(m)) => {
            let mut found: Vec<Vec<u8>> = m.parts().into_iter().map(|p| p.data).filter(|d| audio::sniff(d).is_some()).collect();
            if found.len() != 1 {
                return Some(Err(format!("{} audio parts in the message; {}", found.len(), email_facts(&m))));
            }
            owned = found.pop().unwrap();
            &owned
        }
        // An unrecognised container is not "absent": refuse rather than answer `none`.
        (None, None) => return Some(Err(format!("no audio stream recognised; {}", facts("media", b)))),
    };
    let a = audio::sniff(data).unwrap();
    let opt = |v: Option<String>| v.unwrap_or_else(none);
    Some(Ok(match want {
        "codec-name" | "codec" | "stream-codec-name" => a.codec.clone(),
        "sample-rate" | "sample-rate-hz" => opt(a.sample_rate.map(|v| v.to_string())),
        "channels" | "channel-count" => opt(a.channels.map(|v| v.to_string())),
        "bits-per-sample" | "bit-depth" => opt(a.bits.map(|v| v.to_string())),
        "container" | "format-name" | "container-format" => a.container.to_string(),
        _ => return Some(Err(format!("unknown media field {want:?}; {}", audio_facts(&a, data)))),
    }))
}

/// `...-media-primary-video-<field>` -> the first video stream's field, sniffed from the bytes. Geometry
/// is ffprobe's decoded picture (d18 graded `dimensions` as `WxH`, = SPS cropped size = `stsd` = `tkhd`
/// there); container claims (`display-*`, rotation) are separate fields. Fields other than
/// `dimensions` are unverified. A mail message stands for its single video attachment.
fn video_answer(want: &str, b: &[u8]) -> Result<String, String> {
    let v = match (video::sniff(b), email::Message::parse(b)) {
        (Some(v), _) => v,
        (None, Some(m)) => {
            let mut found: Vec<video::Video> = m.parts().into_iter().filter_map(|p| video::sniff(&p.data)).collect();
            if found.len() != 1 {
                return Err(format!("{} video parts in the message; {}", found.len(), email_facts(&m)));
            }
            found.pop().unwrap()
        }
        (None, None) => return Err(format!("no video stream recognised; {}", facts("media", b))),
    };
    let pair = |p: Option<(u32, u32)>| p.map(|(w, h)| format!("{w}x{h}")).unwrap_or_else(none);
    Ok(match want {
        "dimensions" | "dimensions-pixels" | "resolution" | "frame-size" | "size" | "geometry" => v.dimensions(),
        "width" | "width-pixels" => v.width.to_string(),
        "height" | "height-pixels" => v.height.to_string(),
        "codec-name" | "codec" => v.codec.clone(),
        "coded-dimensions" => pair(v.sps_coded.or(v.entry)),
        "display-dimensions" | "track-dimensions" | "presentation-dimensions" => pair(v.display),
        "rotation" | "rotation-degrees" => v.rotation.map(|r| r.to_string()).unwrap_or_else(none),
        "sample-aspect-ratio" | "sar" => v.sar.map(|(a, b)| format!("{a}:{b}")).unwrap_or_else(|| "1:1".into()),
        "display-aspect-ratio" | "dar" | "aspect-ratio" => v.dar().map(|(a, b)| format!("{a}:{b}")).unwrap_or_else(none),
        "frame-rate" | "fps" => v.frame_rate.map(|(a, b)| if b == 1 { a.to_string() } else { format!("{a}/{b}") }).unwrap_or_else(none),
        "frame-count" | "frames" => v.frames.map(|n| n.to_string()).unwrap_or_else(none),
        "container" | "format-name" | "container-format" => v.container.to_string(),
        _ => return Err(format!("unknown video field {want:?}; {}", video_facts(&v))),
    })
}

fn video_facts(v: &video::Video) -> String {
    format!(
        "video container {} codec {} {} entry {:?} sps coded {:?} crop {:?} display {:?} pixel_crop {:?} matrix {:?} rotation {:?} sar {:?} fps {:?} frames {:?} duration {:?} tags {:?}",
        v.container,
        v.codec,
        v.dimensions(),
        v.entry,
        v.sps_coded,
        v.sps_crop,
        v.display,
        v.pixel_crop,
        v.matrix,
        v.rotation,
        v.sar,
        v.frame_rate,
        v.frames,
        v.duration_seconds,
        v.tags
    )
}

fn audio_facts(a: &audio::Audio, d: &[u8]) -> String {
    let payload = a.payload.and_then(|(s, n)| d.get(s..s + n)).map(|p| hex(&sha256(p)));
    format!(
        "audio container {} codec {} rate {:?} ch {:?} bits {:?} duration {:?} payload sha256 {payload:?} tags {:?}",
        a.container,
        a.codec,
        a.sample_rate,
        a.channels,
        a.bits,
        a.duration_seconds(),
        a.tags
    )
}

/// `...-elf-<field>` -> an ELF identification/header field read from the header alone (d19: "determine
/// architecture directly from the ELF header" - section tables may be absent or bogus, so `Elf::parse` is
/// not used). Byte order comes from EI_DATA (big-endian ELFs store e_machine big-endian). d19 graded
/// `e-machine-u16-hex` as `0x%04x`; other fields are unverified. A file that is not ELF answers `none`.
fn elf_answer(key: &str, b: &[u8]) -> Option<Result<String, String>> {
    let i = key.rfind("-elf-")?;
    let want = &key[i + 5..];
    if want.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let want = want.strip_prefix("elf-").unwrap_or(want);
    if !b.starts_with(b"\x7fELF") {
        return Some(Ok(none()));
    }
    let (class, data, osabi) = (b.get(4).copied(), b.get(5).copied(), b.get(7).copied());
    let e = match data {
        Some(1) => crate::formats::bytes::Endian::Little,
        Some(2) => crate::formats::bytes::Endian::Big,
        other => return Some(Err(format!("ELF EI_DATA {other:?} is neither LSB nor MSB; {}", facts("elf", b)))),
    };
    let u16f = |at: usize| e.u16(b, at).ok_or_else(|| format!("ELF header truncated; {}", facts("elf", b)));
    let entry = || -> Result<u64, String> {
        match class {
            Some(2) => e.u64(b, 24),
            Some(1) => e.u32(b, 24).map(u64::from),
            _ => None,
        }
        .ok_or_else(|| format!("ELF entry not derivable; {}", facts("elf", b)))
    };
    Some(match want {
        "e-machine-u16-hex" | "machine-u16-hex" => u16f(18).map(|m| format!("0x{m:04x}")),
        "e-machine" | "e-machine-u16" | "e-machine-decimal" => u16f(18).map(|m| m.to_string()),
        "e-machine-name" | "architecture" | "arch" => u16f(18).map(|m| elf::machine_name(m).to_string()),
        "e-type-u16-hex" => u16f(16).map(|t| format!("0x{t:04x}")),
        "e-type" | "e-type-u16" => u16f(16).map(|t| t.to_string()),
        "e-entry-hex" | "entry-hex" => entry().map(|v| format!("0x{v:x}")),
        "ei-class" | "class" => Ok(match class { Some(1) => "ELF32".into(), Some(2) => "ELF64".into(), c => format!("{c:?}") }),
        "ei-data" | "endianness" | "byte-order" => Ok(if data == Some(1) { "little".into() } else { "big".into() }),
        "ei-osabi" | "osabi" => Ok(osabi.unwrap_or(0).to_string()),
        _ => Err(format!("unknown ELF field {want:?}; {}", facts("elf", b))),
    })
}

/// `...-macho-<field>` -> a Mach-O architecture field over every slice (thin = one, fat/universal =
/// the fat_arch table, `cafebabe`/`cafebabf`). d20 graded `cpu-type-set` on a thin arm64 file as
/// `["0x0100000c"]` (`0x%08x` lowercase, compact JSON). For a fat file each slice's own header must agree
/// with its table entry ("what truly supports"), else refuse; set = unique values sorted (unverified on
/// multi-slice files). Other fields are unverified guesses.
fn macho_answer(key: &str, b: &[u8]) -> Option<Result<String, String>> {
    let i = key.rfind("-macho-")?;
    let want = &key[i + 7..];
    if want.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let want = want.strip_prefix("macho-").unwrap_or(want);
    let Some(sl) = macho::slices(b) else { return Some(Err(format!("not a Mach-O; {}", facts("macho", b)))) };
    let mut cpus = Vec::new();
    for s in &sl {
        match (s.claimed, s.actual) {
            (_, None) => return Some(Err(format!("Mach-O slice without a Mach-O header {sl:?}; {}", facts("macho", b)))),
            (Some(c), Some(a)) if c.0 != a.0 => return Some(Err(format!("fat table and slice header disagree {sl:?}; {}", facts("macho", b)))),
            (_, Some(a)) => cpus.push(a),
        }
    }
    let set = |f: &dyn Fn(&(u32, u32)) -> String| {
        let mut v: Vec<String> = cpus.iter().map(f).collect();
        v.sort();
        v.dedup();
        compact_json(&v)
    };
    Some(Ok(match want {
        "cpu-type-set" | "cpu-types" | "cputype-set" => set(&|c| format!("0x{:08x}", c.0)),
        "cpu-type-name-set" | "architectures" | "arch-set" => set(&|c| macho::cpu_name(c.0).to_string()),
        "slice-count" | "arch-count" | "nfat-arch" => sl.len().to_string(),
        "is-universal" | "is-fat" => sl[0].claimed.is_some().to_string(),
        _ => return Some(Err(format!("unknown Mach-O field {want:?}; slices {sl:?}; {}", facts("macho", b)))),
    }))
}

/// `...-apk-<field>` -> an Android package field from the binary manifest (AXML; plain-text XML is
/// accepted too), `@0x7f..` references resolved through `resources.arsc`, signer certificates from the
/// JAR signature and the APK Signing Block. d21 graded `package-name` (manifest `package`, verbatim);
/// other fields are unverified. A manifest without the attribute answers `none`; a file that is not a
/// package is refused.
fn apk_answer(key: &str, b: &[u8]) -> Option<Result<String, String>> {
    let i = key.rfind("-apk-")?;
    let want = &key[i + 5..];
    if want.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let want = want.strip_prefix("apk-").unwrap_or(want);
    let Some(a) = apk::Apk::parse(b) else { return Some(Err(format!("not an Android package; {}", facts("apk", b)))) };
    let opt = |v: Option<String>| v.unwrap_or_else(none);
    let cert = a.certs.first().map(|c| c.1.as_slice());
    let info = cert.and_then(apk::cert_info);
    let list = |el: &str| {
        let mut v: Vec<String> = a.manifest.iter().filter(|e| e.name == el).filter_map(|e| e.attr("name").map(|x| x.value.clone())).collect();
        v.sort();
        v.dedup();
        compact_json(&v)
    };
    Some(Ok(match want {
        "package-name" | "package" | "package-id" | "application-id" => opt(a.value("manifest", "package")),
        "version-code" => opt(a.value("manifest", "versionCode")),
        "version-name" => opt(a.value("manifest", "versionName")),
        "min-sdk" | "min-sdk-version" => opt(a.value("uses-sdk", "minSdkVersion")),
        "target-sdk" | "target-sdk-version" => opt(a.value("uses-sdk", "targetSdkVersion")),
        "compile-sdk" | "compile-sdk-version" => opt(a.value("manifest", "compileSdkVersion")),
        "app-label" | "application-label" | "label" | "app-name" | "application-name" | "product-name" => opt(a.value("application", "label")),
        "launcher-activity" | "main-activity" | "launchable-activity" => opt(a.launcher_activity()),
        "permissions" | "uses-permissions" | "permission-set" => list("uses-permission"),
        "signer-sha256" | "signer-cert-sha256" | "cert-sha256" | "certificate-sha256" | "signing-cert-sha256" | "signer-certificate-sha256" => opt(cert.map(|c| hex(&sha256(c)))),
        "signer-sha1" | "cert-sha1" | "certificate-sha1" | "signing-cert-sha1" | "signer-cert-sha1" => opt(cert.map(|c| hex(&sha1(c)))),
        "signer-subject" | "cert-subject" | "certificate-subject" | "signer-dn" => opt(info.as_ref().map(|i| apk::dn_string(&i.subject))),
        "signer-issuer" | "cert-issuer" | "certificate-issuer" => opt(info.as_ref().map(|i| apk::dn_string(&i.issuer))),
        "signer-cn" | "cert-cn" | "signer-common-name" => opt(info.as_ref().and_then(|i| i.subject.iter().find(|k| k.0 == "CN").map(|k| k.1.clone()))),
        "signer-serial" | "cert-serial" | "cert-serial-hex" => opt(info.as_ref().map(|i| i.serial_hex.clone())),
        _ => return Some(Err(format!("unknown APK field {want:?}; {}", apk_facts(&a)))),
    }))
}

fn apk_facts(a: &apk::Apk) -> String {
    let els: Vec<String> = a.manifest.iter().map(|e| format!("{}{} {:?}", "  ".repeat(e.depth), e.name, e.attrs.iter().map(|x| format!("{}={}", x.name, x.value)).collect::<Vec<_>>())).collect();
    let certs: Vec<String> = a
        .certs
        .iter()
        .map(|(src, c)| {
            let i = apk::cert_info(c);
            format!("{src}: sha256 {} subject {:?} issuer {:?} serial {:?}", hex(&sha256(c)), i.as_ref().map(|i| apk::dn_string(&i.subject)), i.as_ref().map(|i| apk::dn_string(&i.issuer)), i.as_ref().map(|i| i.serial_hex.clone()))
        })
        .collect();
    format!("apk binary_manifest {} members {:?} manifest {els:?} arsc {} certs {certs:?}", a.binary_manifest, a.members, a.arsc.is_some())
}

/// `...-jar-<field>` -> a Java archive field. d22 graded `main-class`: the manifest's `Main-Class`,
/// verbatim, `none` when the attribute is absent. The named class must exist in the archive and declare
/// `public static void main(String[])`; a manifest that disagrees with its classes is an error (the graded
/// rule for that case is unknown). Other fields are unverified; a field named like a manifest attribute
/// (`implementation-version`) answers that attribute.
fn jar_answer(key: &str, b: &[u8]) -> Option<Result<String, String>> {
    let i = key.rfind("-jar-")?;
    let want = &key[i + 5..];
    if want.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let want = want.strip_prefix("jar-").unwrap_or(want);
    let want = want.strip_prefix("manifest-").unwrap_or(want);
    let Some(j) = jar::Jar::parse(b) else { return Some(Err(format!("not a Java archive; {}", facts("jar", b)))) };
    let launcher = |attr: &str| -> Result<String, String> {
        match j.attr(attr) {
            None => Ok(none()),
            Some(v) => match j.class(v) {
                Some(c) if c.has_main => Ok(v.trim().to_string()),
                _ => Err(format!("{attr} {v:?} has no launchable class in the archive; {}", jar_facts(&j))),
            },
        }
    };
    Some(match want {
        "main-class" | "entry-point" | "main" => launcher("Main-Class"),
        "start-class" => launcher("Start-Class"),
        "is-runnable" | "runnable" | "is-executable" | "executable" => {
            j.attr("Main-Class").map_or(Ok(false.to_string()), |v| match j.class(v) {
                Some(c) if c.has_main => Ok(true.to_string()),
                _ => Err(format!("Main-Class {v:?} has no launchable class; {}", jar_facts(&j))),
            })
        }
        "class-count" => Ok(j.classes.len().to_string()),
        "nested-jars" | "nested-jar-names" | "nested-dependencies" | "dependencies" => {
            let mut v: Vec<String> = j.nested.iter().map(|n| n.0.clone()).collect();
            v.sort();
            Ok(compact_json(&v))
        }
        other => match j.manifest.as_ref().and_then(|m| m.main.iter().find(|a| a.0.eq_ignore_ascii_case(other) || a.0.replace('_', "-").eq_ignore_ascii_case(&other.replace('_', "-")))) {
            Some(a) => Ok(a.1.trim().to_string()),
            None => Err(format!("unknown JAR field {other:?}; {}", jar_facts(&j))),
        },
    })
}

fn jar_facts(j: &jar::Jar) -> String {
    let cls: Vec<String> = j.classes.iter().map(|c| format!("{}{}{}", c.path, c.this_class.as_ref().filter(|t| format!("{t}.class") != c.path).map(|t| format!(" (this {t})")).unwrap_or_default(), if c.has_main { " +main" } else { "" })).collect();
    let nested: Vec<String> = j.nested.iter().map(|(n, s)| format!("{n}: [{}]", jar_facts(s))).collect();
    format!("jar manifest {:?} sections {} classes {cls:?} members {:?} nested {nested:?}", j.manifest.as_ref().map(|m| &m.main), j.manifest.as_ref().map_or(0, |m| m.sections.len()), j.members)
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
    } else if b.starts_with(b"\x7fELF") {
        f.push(format!("elf header only (sections unparseable): ident {}", hex(&b[..b.len().min(24)])));
    }
    if let Some(sl) = macho::slices(b) {
        f.push(format!("macho slices {sl:?}"));
    }
    if let Some(props) = cfb::msi_properties(b) {
        f.push(format!("msi {props:?} revision {:?}", cfb::summary_string(b, 9)));
    }
    if let Some(a) = audio::sniff(b) {
        f.push(audio_facts(&a, b));
    }
    if let Some(v) = video::sniff(b) {
        f.push(video_facts(&v));
    }
    if let Some(m) = email::Message::parse(b) {
        f.push(email_facts(&m));
    }
    if let Some(a) = apk::Apk::parse(b) {
        f.push(apk_facts(&a));
    }
    if let Some(j) = jar::Jar::parse(b).filter(|j| j.manifest.is_some() || !j.classes.is_empty()) {
        f.push(jar_facts(&j));
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
        if key.contains("-media-") {
            let data = data.ok_or_else(|| format!("no file {member:?} for {label:?}"))?;
            return media_answer(&key, data).unwrap_or_else(|| Err(format!("unknown kind {key:?}; {}", facts(member, data))));
        }
        // Email fields go first: `-email-attachment-sha256` / `-filename` must not hit the whole-file handlers.
        if key.contains("-email-") {
            let data = data.ok_or_else(|| format!("no file {member:?} for {label:?}"))?;
            return email_answer(&key, data).unwrap_or_else(|| Err(format!("unknown kind {key:?}; {}", facts(member, data))));
        }
        if key.ends_with("-filename") && !key.ends_with("original-filename") {
            if !member.is_empty() {
                return Ok(member.to_string());
            }
            return filename_answer(members, &key).ok_or_else(|| format!("no member matches {key:?} in {:?}", members.iter().map(|m| &m.0).collect::<Vec<_>>()));
        }
        let data = data.ok_or_else(|| format!("no file {member:?} for {label:?} (members {:?})", members.iter().map(|m| &m.0).collect::<Vec<_>>()))?;
        if let Some(r) = elf_answer(&key, data) {
            return r;
        }
        if let Some(r) = macho_answer(&key, data) {
            return r;
        }
        if let Some(r) = apk_answer(&key, data) {
            return r;
        }
        if let Some(r) = jar_answer(&key, data) {
            return r;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    const EML: &[u8] = b"From: a <a@x>\r\nMessage-ID: <id@x>\r\nContent-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nhello\r\n--b\r\nContent-Disposition: attachment; filename=\"n.txt\"\r\nContent-Transfer-Encoding: base64\r\n\r\naGk=\r\n--b--\r\n";

    #[test]
    fn media_fields() {
        let mut w = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\x02\0\0\0\x01\x02".to_vec();
        assert_eq!(media_answer("d17-audio-001-media-primary-audio-codec-name", &w).unwrap().unwrap(), "pcm_s16le");
        assert_eq!(media_answer("d17-audio-001-media-primary-audio-sample-rate", &w).unwrap().unwrap(), "16000");
        w[20] = 7;
        assert_eq!(media_answer("d17-audio-001-media-primary-audio-codec-name", &w).unwrap().unwrap(), "pcm_mulaw");
        assert!(media_answer("d17-audio-001-media-primary-audio-codec-name", b"plain text").unwrap().is_err());
    }

    #[test]
    fn elf_fields() {
        let mut h = b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0\x02\0\x3e\0".to_vec();
        let k = "d19-elf-001-elf-e-machine-u16-hex";
        assert_eq!(elf_answer(k, &h).unwrap().unwrap(), "0x003e");
        h[5] = 2;
        h[18..20].copy_from_slice(&[0, 8]);
        assert_eq!(elf_answer(k, &h).unwrap().unwrap(), "0x0008");
        assert_eq!(elf_answer(k, b"MZ\x90\0").unwrap().unwrap(), "none");
        assert!(elf_answer("d19-elf-001-sha256", &h).is_none());
    }

    #[test]
    fn macho_fields() {
        let k = "d20-macho-001-macho-cpu-type-set";
        let thin = b"\xcf\xfa\xed\xfe\x0c\0\0\x01\0\0\0\0\x02\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        assert_eq!(macho_answer(k, thin).unwrap().unwrap(), r#"["0x0100000c"]"#);
        let mut fat = b"\xca\xfe\xba\xbe\0\0\0\x02".to_vec();
        fat.extend_from_slice(b"\x01\0\0\x0c\0\0\0\0\0\0\0\x40\0\0\0\x20\0\0\0\0");
        fat.extend_from_slice(b"\x01\0\0\x07\0\0\0\x03\0\0\0\x60\0\0\0\x20\0\0\0\0");
        fat.resize(0x40, 0);
        fat.extend_from_slice(thin);
        let mut x86 = thin.to_vec();
        x86[4] = 7;
        fat.extend_from_slice(&x86);
        assert_eq!(macho_answer(k, &fat).unwrap().unwrap(), r#"["0x01000007","0x0100000c"]"#);
        fat[0x60 + 4] = 0x0c;
        assert!(macho_answer(k, &fat).unwrap().is_err());
        assert!(macho_answer("d20-macho-001-sha256", thin).is_none());
    }

    #[test]
    fn email_fields() {
        let a = |k: &str| email_answer(k, EML).unwrap();
        assert_eq!(a("d16-email-001-email-message-id").unwrap(), "<id@x>");
        assert_eq!(a("d17-email-001-email-attachment-filename").unwrap(), "n.txt");
        assert_eq!(a("d17-email-001-email-attachment-sha256").unwrap(), hex(&sha256(b"hi")));
        assert_eq!(a("d17-email-001-email-in-reply-to").unwrap(), "none");
        assert!(a("d17-email-001-email-thread-root").is_err());
    }
    #[test]
    fn apk_fields() {
        let b = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/d21-mobile-001.apk")).unwrap();
        let a = |k: &str| apk_answer(k, &b).unwrap();
        assert_eq!(a("d21-apk-001-apk-package-name").unwrap(), "com.huntress.arena.p08281778");
        assert_eq!(a("d21-apk-001-apk-app-label").unwrap(), "Huntress Evidence 08281778");
        eprintln!("{}", a("d21-apk-001-apk-dump-facts").unwrap_err());
        assert!(apk_answer("d21-apk-001-sha256", &b).is_none());
    }

    #[test]
    fn jar_fields() {
        let b = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/d22-application-001.jar")).unwrap();
        let a = |k: &str| jar_answer(k, &b).unwrap();
        assert_eq!(a("d22-jar-001-jar-main-class").unwrap(), "arena.evidence.ArtifactMain");
        assert_eq!(a("d22-jar-001-jar-created-by").unwrap(), "Huntress Artifact Assembly Line");
        assert_eq!(a("d22-jar-001-jar-start-class").unwrap(), "none");
        assert!(jar_answer("d22-jar-001-sha256", &b).is_none());
        let m = jar::Manifest::parse(b"Manifest-Version: 1.0\r\nMain-Class: a.b\r\n .C\r\n\r\nName: x\r\nK: v\r\n");
        assert_eq!(m.get("main-class"), Some("a.b.C"));
        assert_eq!(m.sections.len(), 1);
    }
}
