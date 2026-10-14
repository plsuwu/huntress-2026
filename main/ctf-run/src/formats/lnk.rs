//! Windows Shell Link (`.lnk`, MS-SHLLINK): header, LinkInfo paths, StringData, and the
//! environment-variable / tracker extra blocks. Every field is exposed by name so a question kind
//! can pick one, and the whole set doubles as a FACTS dump.

use super::bytes::{cstr, u16le, u32le, u64le, utf16le};
use std::collections::BTreeMap;

const CLSID: [u8; 16] = [0x01, 0x14, 0x02, 0, 0, 0, 0, 0, 0xC0, 0, 0, 0, 0, 0, 0, 0x46];

pub fn is_lnk(d: &[u8]) -> bool {
    u32le(d, 0) == Some(0x4C) && d.get(4..20) == Some(&CLSID)
}

fn ansi(b: &[u8]) -> String {
    super::bytes::cp1252(b)
}

/// "My Computer" ({20D04FE0-3AEA-1069-A2D8-08002B30309D}) as stored in a root shell item.
const MY_COMPUTER: [u8; 16] = [0xE0, 0x4F, 0xD0, 0x20, 0xEA, 0x3A, 0x69, 0x10, 0xA2, 0xD8, 0x08, 0x00, 0x2B, 0x30, 0x30, 0x9D];

/// The absolute path a LinkTargetIDList spells: My Computer root, a volume item (`C:\`), then file
/// entries (long name from the 0xBEEF0004 extension, else the short name). `None` for anything else
/// (other roots, unknown item classes), so the caller never compares a partial path.
fn idlist_path(list: &[u8]) -> Option<String> {
    let mut q = 0;
    let mut drive: Option<String> = None;
    let mut parts: Vec<String> = Vec::new();
    while q + 2 <= list.len() {
        let isz = u16le(list, q)? as usize;
        if isz == 0 {
            break;
        }
        let item = list.get(q..q + isz)?;
        let class = *item.get(2)?;
        match class & 0x70 {
            0x10 if class == 0x1F => {
                if item.get(4..20)? != MY_COMPUTER || drive.is_some() || !parts.is_empty() {
                    return None;
                }
            }
            0x20 => {
                let name = ansi(cstr(item, 3)?);
                if drive.is_some() || !parts.is_empty() || name.len() < 2 || name.as_bytes()[1] != b':' {
                    return None;
                }
                drive = Some(if name.ends_with('\\') { name } else { name + "\\" });
            }
            0x30 => {
                drive.as_ref()?;
                parts.push(file_entry_name(item)?);
            }
            _ => return None,
        }
        q += isz;
    }
    let drive = drive?;
    Some(drive + &parts.join("\\"))
}

/// A file-entry shell item's long name, only when it is provably the item's name: the 0xBEEF0004 long
/// name must match the short name (equal ignoring case, or the 8.3 alias's prefix before `~`). A plain
/// short name stands in when there is no extension block; a mangled alias (`PROGRA~1`) never does. `None`
/// drops the whole IDList path, so a misparsed block can never contradict LinkInfo.
fn file_entry_name(item: &[u8]) -> Option<String> {
    let short = ansi(cstr(item, 14)?);
    let sig = 0xBEEF_0004u32.to_le_bytes();
    let at = item.windows(4).position(|w| w == sig).and_then(|i| i.checked_sub(4));
    let long = at.and_then(|b| {
        let ver = u16le(item, b + 2)?;
        // Block layout (libfwsi; v3 confirmed on day-14 bytes): +16 identifier, v7+: +18 unknown, +20 file
        // reference, +28 unknown; then a u16 long-string size, v8+/v9+ add 4 bytes each, then the UTF-16 name.
        let off = match ver {
            3..=6 => 20,
            7 => 38,
            8 => 42,
            _ => 46,
        };
        let s = item.get(b + off..)?;
        let end = s.chunks_exact(2).position(|c| c == [0, 0])? * 2;
        let name = utf16le(&s[..end]);
        (!name.is_empty() && name.chars().all(|c| !c.is_control())).then_some(name)
    });
    match (long, short.contains('~')) {
        (Some(l), false) if l.eq_ignore_ascii_case(&short) => Some(l),
        (Some(l), true) if alias_matches(&short, &l) => Some(l),
        (None, false) if at.is_none() && !short.is_empty() => Some(short),
        _ => None,
    }
}

/// `PROGRA~1` vs `Program Files`: the alias stem (before `~`) is the uppercased long name with spaces
/// and dots removed, truncated.
fn alias_matches(short: &str, long: &str) -> bool {
    let stem = short.split('~').next().unwrap_or("");
    let squashed: String = long.chars().filter(|c| *c != ' ' && *c != '.').collect::<String>().to_ascii_uppercase();
    !stem.is_empty() && squashed.starts_with(&stem.to_ascii_uppercase())
}

/// Parses a shell link into named fields (empty fields are omitted).
pub fn parse(d: &[u8]) -> Option<BTreeMap<&'static str, String>> {
    if !is_lnk(d) {
        return None;
    }
    let mut f = BTreeMap::new();
    let flags = u32le(d, 0x14)?;
    f.insert("link_flags", format!("0x{flags:08x}"));
    f.insert("file_attributes", format!("0x{:08x}", u32le(d, 0x18)?));
    f.insert("creation_time", u64le(d, 0x1C)?.to_string());
    f.insert("access_time", u64le(d, 0x24)?.to_string());
    f.insert("write_time", u64le(d, 0x2C)?.to_string());
    f.insert("file_size", u32le(d, 0x34)?.to_string());
    f.insert("icon_index", (u32le(d, 0x38)? as i32).to_string());
    f.insert("show_command", u32le(d, 0x3C)?.to_string());
    f.insert("hotkey", format!("0x{:04x}", u16le(d, 0x40)?));
    let unicode = flags & 0x80 != 0;
    let mut p = 0x4C;
    if flags & 0x01 != 0 {
        // LinkTargetIDList: keep the readable item names (best effort).
        let size = u16le(d, p)? as usize;
        let list = d.get(p + 2..p + 2 + size)?;
        let mut names = Vec::new();
        let mut q = 0;
        while q + 2 <= list.len() {
            let isz = u16le(list, q)? as usize;
            if isz < 2 {
                break;
            }
            if let Some(item) = list.get(q + 2..q + isz) {
                let txt: String = item.iter().map(|&c| if c.is_ascii_graphic() || c == b' ' { c as char } else { '\0' }).collect();
                if let Some(best) = txt.split('\0').filter(|s| s.len() >= 2).max_by_key(|s| s.len()) {
                    names.push(best.to_string());
                }
            }
            q += isz;
        }
        f.insert("idlist_items", names.join(" | "));
        if let Some(path) = idlist_path(list) {
            f.insert("idlist_path", path);
        }
        p += 2 + size;
    }
    if flags & 0x02 != 0 {
        let li = d.get(p..)?;
        let size = u32le(li, 0)? as usize;
        let hsize = u32le(li, 4)? as usize;
        let lflags = u32le(li, 8)?;
        let vol = u32le(li, 12)? as usize;
        let lbp = u32le(li, 16)? as usize;
        let cnrl = u32le(li, 20)? as usize;
        let cps = u32le(li, 24)? as usize;
        if lflags & 1 != 0 {
            if let Some(b) = cstr(li, lbp) {
                f.insert("local_base_path", ansi(b));
            }
            if vol > 0 {
                f.insert("drive_type", u32le(li, vol + 4).map(|v| v.to_string()).unwrap_or_default());
                f.insert("drive_serial", u32le(li, vol + 8).map(|v| format!("0x{v:08x}")).unwrap_or_default());
                let lo = u32le(li, vol + 12).unwrap_or(0) as usize;
                if let Some(b) = cstr(li, vol + lo) {
                    f.insert("volume_label", ansi(b));
                }
            }
        }
        if lflags & 2 != 0 && cnrl > 0 {
            let c = cnrl;
            let net = u32le(li, c + 8).unwrap_or(0) as usize;
            let dev = u32le(li, c + 12).unwrap_or(0) as usize;
            if let Some(b) = cstr(li, c + net) {
                f.insert("net_name", ansi(b));
            }
            if dev > 0 {
                if let Some(b) = cstr(li, c + dev) {
                    f.insert("device_name", ansi(b));
                }
            }
        }
        if let Some(b) = cstr(li, cps) {
            f.insert("common_path_suffix", ansi(b));
        }
        if hsize >= 0x24 {
            let lbpu = u32le(li, 28).unwrap_or(0) as usize;
            let cpsu = u32le(li, 32).unwrap_or(0) as usize;
            let w = |o: usize| -> Option<String> {
                let s = li.get(o..)?;
                let end = s.chunks_exact(2).position(|c| c == [0, 0])? * 2;
                Some(utf16le(&s[..end]))
            };
            if lbpu > 0 {
                if let Some(s) = w(lbpu) {
                    f.insert("local_base_path", s);
                }
            }
            if cpsu > 0 {
                if let Some(s) = w(cpsu) {
                    f.insert("common_path_suffix", s);
                }
            }
        }
        // The full target: base path + suffix, or share + suffix.
        let base = f.get("local_base_path").or(f.get("net_name")).cloned().unwrap_or_default();
        let suffix = f.get("common_path_suffix").cloned().unwrap_or_default();
        let target = if suffix.is_empty() {
            base
        } else if base.ends_with('\\') || base.is_empty() {
            base + &suffix
        } else {
            base + "\\" + &suffix
        };
        if !target.is_empty() {
            f.insert("target_path", target);
        }
        p += size;
    }
    for (bit, name) in [(0x04, "name"), (0x08, "relative_path"), (0x10, "working_dir"), (0x20, "arguments"), (0x40, "icon_location")] {
        if flags & bit != 0 {
            let n = u16le(d, p)? as usize;
            let s = if unicode { utf16le(d.get(p + 2..p + 2 + n * 2)?) } else { ansi(d.get(p + 2..p + 2 + n)?) };
            p += 2 + if unicode { n * 2 } else { n };
            f.insert(name, s);
        }
    }
    // ExtraData blocks.
    while let Some(size) = u32le(d, p).map(|s| s as usize) {
        if size < 8 {
            break;
        }
        let sig = u32le(d, p + 4)?;
        let b = d.get(p..p + size)?;
        match sig {
            0xA000_0001 | 0xA000_0007 => {
                let key = if sig == 0xA000_0001 { "environment_target" } else { "icon_environment_target" };
                let uni = b.get(268..788).map(|u| {
                    let end = u.chunks_exact(2).position(|c| c == [0, 0]).unwrap_or(u.len() / 2) * 2;
                    utf16le(&u[..end])
                });
                let a = cstr(b, 8).map(ansi);
                if let Some(s) = uni.filter(|s| !s.is_empty()).or(a) {
                    f.insert(key, s);
                }
            }
            0xA000_0003 => {
                if let Some(m) = cstr(b, 16) {
                    f.insert("machine_id", ansi(m));
                }
            }
            0xA000_0005 => {
                f.insert("special_folder_id", u32le(b, 8).map(|v| v.to_string()).unwrap_or_default());
            }
            0xA000_0006 => {
                if let Some(m) = cstr(b, 8) {
                    f.insert("darwin_id", ansi(m));
                }
            }
            _ => {}
        }
        p += size;
    }
    f.retain(|_, v| !v.is_empty());
    Some(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal link built by hand: LinkInfo with a local base path, then ARGUMENTS + WORKING_DIR (unicode).
    #[test]
    fn parses_core_fields() {
        let mut d = vec![0u8; 0x4C];
        d[0..4].copy_from_slice(&0x4Cu32.to_le_bytes());
        d[4..20].copy_from_slice(&CLSID);
        let flags: u32 = 0x02 | 0x10 | 0x20 | 0x80;
        d[0x14..0x18].copy_from_slice(&flags.to_le_bytes());
        // LinkInfo: header 0x1C, VolumeID at 0x1C, base path after it, suffix "" after that.
        let mut li = vec![0u8; 0x1C];
        let vol = vec![0x11, 0, 0, 0, 3, 0, 0, 0, 0x78, 0x56, 0x34, 0x12, 0x10, 0, 0, 0, 0];
        let base = b"C:\\Windows\\System32\\cmd.exe\0";
        let lbp = 0x1C + vol.len();
        let cps = lbp + base.len();
        li.extend_from_slice(&vol);
        li.extend_from_slice(base);
        li.push(0);
        let size = li.len() as u32;
        li[0..4].copy_from_slice(&size.to_le_bytes());
        li[4..8].copy_from_slice(&0x1Cu32.to_le_bytes());
        li[8..12].copy_from_slice(&1u32.to_le_bytes());
        li[12..16].copy_from_slice(&0x1Cu32.to_le_bytes());
        li[16..20].copy_from_slice(&(lbp as u32).to_le_bytes());
        li[24..28].copy_from_slice(&(cps as u32).to_le_bytes());
        d.extend_from_slice(&li);
        for s in ["C:\\Users\\Public", "/c whoami"] {
            let u: Vec<u16> = s.encode_utf16().collect();
            d.extend_from_slice(&(u.len() as u16).to_le_bytes());
            d.extend(u.iter().flat_map(|x| x.to_le_bytes()));
        }
        d.extend_from_slice(&[0, 0, 0, 0]);
        let f = parse(&d).unwrap();
        assert_eq!(f["target_path"], "C:\\Windows\\System32\\cmd.exe");
        assert_eq!(f["working_dir"], "C:\\Users\\Public");
        assert_eq!(f["arguments"], "/c whoami");
        assert_eq!(f["drive_serial"], "0x12345678");
    }

    /// Day 14 single-solve shortcut: My Computer / C:\ / file entries with v3 0xBEEF0004 long names.
    #[test]
    fn idlist_path_from_real_link() {
        let f = parse(&std::fs::read("testdata/d14-shortcut-001.lnk").unwrap()).unwrap();
        assert_eq!(f["idlist_path"], "C:\\Windows\\System32\\cmd.exe");
        assert_eq!(f["target_path"], "C:\\Windows\\System32\\cmd.exe");
        // The long names come from the v3 extension blocks, not the short-name fallback.
        let d = std::fs::read("testdata/d14-shortcut-001.lnk").unwrap();
        let item = &d[0x7B..0x7B + 0x3C]; // the "Windows" file entry
        assert_eq!(item[2], 0x31);
        let mut no_short = item.to_vec();
        no_short[14] = b'X'; // "Xindows": the long name no longer matches -> dropped, never substituted
        assert_eq!(file_entry_name(item).as_deref(), Some("Windows"));
        assert_eq!(file_entry_name(&no_short), None);
    }

    /// A file-entry shell item with an 0xBEEF0004 block of the given version.
    fn file_entry(short: &str, long: &str, ver: u16) -> Vec<u8> {
        let mut it = vec![0, 0, 0x31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10, 0];
        it.extend_from_slice(short.as_bytes());
        it.push(0);
        if it.len() % 2 == 1 {
            it.push(0);
        }
        let start = it.len();
        let mut b = vec![0, 0];
        b.extend_from_slice(&ver.to_le_bytes());
        b.extend_from_slice(&0xBEEF_0004u32.to_le_bytes());
        b.extend_from_slice(&[0; 8]); // creation + access
        b.extend_from_slice(&[0x14, 0]); // identifier
        if ver >= 7 {
            b.extend_from_slice(&[0; 18]); // unknown, file reference, unknown
        }
        b.extend_from_slice(&[0, 0]); // long string size
        if ver >= 8 {
            b.extend_from_slice(&[0; 4]);
        }
        if ver >= 9 {
            b.extend_from_slice(&[0; 4]);
        }
        b.extend(long.encode_utf16().flat_map(|u| u.to_le_bytes()));
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&(start as u16).to_le_bytes());
        let n = b.len() as u16;
        b[0..2].copy_from_slice(&n.to_le_bytes());
        it.extend_from_slice(&b);
        let n = it.len() as u16;
        it[0..2].copy_from_slice(&n.to_le_bytes());
        it
    }

    #[test]
    fn long_names_by_extension_version() {
        for ver in [3, 7, 8, 9] {
            assert_eq!(file_entry_name(&file_entry("PROGRA~1", "Program Files", ver)).as_deref(), Some("Program Files"), "v{ver}");
            assert_eq!(file_entry_name(&file_entry("cmd.exe", "cmd.exe", ver)).as_deref(), Some("cmd.exe"), "v{ver}");
            // Alias and long name disagree -> unusable, not a guess.
            assert_eq!(file_entry_name(&file_entry("WINDOW~1", "Program Files", ver)), None, "v{ver}");
        }
        // A misread version (v9 layout labelled v3) yields no validated name -> the IDList path is dropped.
        let mut bad = file_entry("PROGRA~1", "Program Files", 9);
        let b = bad.windows(4).position(|w| w == 0xBEEF_0004u32.to_le_bytes()).unwrap() - 4;
        bad[b + 2] = 3;
        assert_eq!(file_entry_name(&bad), None);
        // No extension block at all: a plain short name stands in, a mangled alias does not.
        let mut plain = vec![0, 0, 0x32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x20, 0];
        plain.extend_from_slice(b"cmd.exe\0");
        assert_eq!(file_entry_name(&plain).as_deref(), Some("cmd.exe"));
        let mut mangled = plain[..14].to_vec();
        mangled.extend_from_slice(b"PROGRA~1\0");
        assert_eq!(file_entry_name(&mangled), None);
    }
}
