//! The smaller specimen families: PowerShell samples, VBScript control flow, advanced Bash, and the
//! polyglot / host descriptor literals.

use super::descriptor::{decode_literal, fields};
use super::labelled;
use crate::json::Value;
use crate::re;
use std::collections::BTreeMap;

// ---- PowerShell (`FromBase64String('...')` JSON; keys shuffled, values classified by shape) ----

#[derive(Clone, Debug, Default)]
pub struct Ps1 {
    pub fields: BTreeMap<&'static str, String>,
    pub files: Vec<String>,
}

pub fn analyse_ps1(text: &str) -> Result<Ps1, String> {
    let m = re!(r"FromBase64String\('([A-Za-z0-9+/=]+)'\)").search(text).ok_or("no base64 literal")?;
    let raw = crate::codec::base64(m.g(1)).ok_or("bad base64")?;
    if let Some(b) = re!(r"\$reconstruct\s*=\s*\{(.*?)\}\s*\n", "s").search(text) {
        if !re!(r"return\s+\[byte\[\]\]\$candidateBytes").is_match(b.g(1)) {
            return Err("non-identity $reconstruct block".into());
        }
    }
    let rec = Value::parse_bytes(&raw).map_err(|e| e.to_string())?;
    let mut out = Ps1::default();
    for (_, v) in rec.as_object().ok_or("record is not an object")? {
        match v {
            Value::Arr(a) => out.files = a.iter().filter_map(Value::as_str).map(str::to_string).collect(),
            Value::Str(s) => {
                let key = if s.starts_with("https://") {
                    if out.fields.contains_key("url") {
                        continue;
                    }
                    "url"
                } else if s.starts_with("wallet_") {
                    "wallet"
                } else if s.starts_with("Global\\") {
                    "mutex"
                } else if s.starts_with("Mozilla/") {
                    "user_agent"
                } else if s.starts_with('.') {
                    "extension"
                } else if s.ends_with(".txt") {
                    "note"
                } else {
                    continue;
                };
                out.fields.insert(key, s.clone());
            }
            _ => {}
        }
    }
    let (scenario, attack) = if out.fields.contains_key("wallet") {
        ("resource_hijacking", "T1496")
    } else if out.fields.contains_key("extension") && out.fields.contains_key("note") {
        ("synthetic_impact", "T1486")
    } else if out.files.iter().any(|f| f == "Login Data") {
        ("synthetic_collection", "T1555.003")
    } else {
        return Err("unclassified PowerShell sample".into());
    };
    out.fields.insert("scenario", scenario.into());
    out.fields.insert("attack", attack.into());
    Ok(out)
}

pub fn answer_ps1(key: &str, a: &Ps1) -> Result<String, String> {
    if key.ends_with("-file") {
        return a.files.first().cloned().ok_or_else(|| "no files".into());
    }
    for (suffix, k) in [("-scenario", "scenario"), ("-attack", "attack"), ("-url", "url"), ("-wallet", "wallet"), ("-mutex", "mutex"), ("-user-agent", "user_agent"), ("-extension", "extension"), ("-note", "note")] {
        if key.ends_with(suffix) {
            return a.fields.get(k).cloned().ok_or_else(|| format!("{key}: {k} not in sample"));
        }
    }
    Err(format!("ps1 question {key}"))
}

// ---- VBScript: control wrapper around the Decode_ call (lexical) ----

fn strip_vbs_comment(line: &str) -> &str {
    let mut in_str = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_str = !in_str,
            '\'' if !in_str => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Innermost loop/Select block still open at the `Decode_<sfx>(` call, else `error_guard` when
/// `On Error Resume Next` is active, else `direct`.
pub fn vbs_control(text: &str) -> Result<String, String> {
    let sfx = re!(r"(?im)^\s*Function\s+Recover_(\w+)").search(text).ok_or("no Recover_ function")?.g(1).to_string();
    let mut in_recover = false;
    let mut stack: Vec<&str> = Vec::new();
    let mut on_error = false;
    let call = crate::rx::Regex::with_flags(&format!(r"\bDecode_{sfx}\s*\("), "i").unwrap();
    for raw in text.lines() {
        let t = strip_vbs_comment(raw).trim();
        if !in_recover {
            if re!(r"(?i)^(?:Function|Sub)\s+Recover_").match_(t).is_some() {
                in_recover = true;
            }
            continue;
        }
        if re!(r"(?i)^End\s+(?:Function|Sub)\b").match_(t).is_some() {
            break;
        }
        let low = t.to_lowercase();
        if low.is_empty() {
            continue;
        }
        if call.is_match(t) {
            let wrappers: Vec<&str> = stack.iter().copied().filter(|s| *s != "if_block").collect();
            return Ok(match (wrappers.last(), on_error) {
                (Some(w), _) => w.to_string(),
                (None, true) => "error_guard".into(),
                (None, false) => "direct".into(),
            });
        }
        let pop = |stack: &mut Vec<&str>, kind: &str| {
            if let Some(j) = stack.iter().rposition(|s| *s == kind) {
                stack.remove(j);
            }
        };
        if low.starts_with("on error resume next") {
            on_error = true;
        } else if low.starts_with("on error goto 0") {
            on_error = false;
        } else if re!(r"^for\b(?!\s+each)").match_(&low).is_some() {
            stack.push("for_once");
        } else if re!(r"^next\b").match_(&low).is_some() {
            pop(&mut stack, "for_once");
        } else if re!(r"^do\b").match_(&low).is_some() {
            stack.push("do_once");
        } else if re!(r"^loop\b").match_(&low).is_some() {
            pop(&mut stack, "do_once");
        } else if re!(r"^while\b").match_(&low).is_some() {
            stack.push("while_once");
        } else if re!(r"^wend\b").match_(&low).is_some() {
            pop(&mut stack, "while_once");
        } else if low.starts_with("select case") {
            stack.push("select_case");
        } else if low.starts_with("end select") {
            pop(&mut stack, "select_case");
        } else if re!(r"^if\b.*\bthen\s*$").match_(&low).is_some() {
            stack.push("if_block");
        } else if re!(r"^end if\b").match_(&low).is_some() {
            pop(&mut stack, "if_block");
        }
    }
    Err("no Decode call in Recover".into())
}

pub fn is_vbs(text: &str) -> bool {
    re!(r"(?im)^\s*Function\s+Recover_\w+").is_match(text) && re!(r"(?im)^\s*Const\s+Material_").is_match(text)
}

// ---- Advanced Bash: `v=1|feature=..|primitive=..|campaign_id=..|stage_count=N|steps=a,b` ----

pub fn analyse_adv_bash(text: &str) -> Result<BTreeMap<String, String>, String> {
    let body = re!(r"(?m)^\s*Decode\(\)\s*\{(.*)\}\s*$").search(text).map(|m| m.g(1).to_string()).unwrap_or_else(|| text.to_string());
    let chunks = re!(r"readonly\s+Material_\w+=((?:'[^']*'\s*)+)").search(text).ok_or("no Material_")?;
    let material: String = re!(r"'([^']*)'").findall(chunks.g(1)).concat();
    let valid = |f: &BTreeMap<String, String>| f.get("v").is_some() && (f.contains_key("feature") || f.contains_key("steps"));
    // Day 8+: `$((16#.. ^ 16#kk))` xor over decimal-csv or hex-pair material.
    if let Some(xk) = re!(r"\^\s*16#([0-9a-fA-F]{1,2})\b").search(&body) {
        let k = u8::from_str_radix(xk.g(1), 16).unwrap();
        let data: Vec<u8> = if re!(r"\d+(?:,\d+)*").fullmatch(&material).is_some() {
            material.split(',').filter_map(|x| x.parse::<u16>().ok()).map(|x| x as u8).collect()
        } else {
            crate::codec::hex(&material).ok_or("advanced bash material form unknown")?
        };
        let f = fields(&String::from_utf8_lossy(&data.iter().map(|b| b ^ k).collect::<Vec<u8>>()));
        if valid(&f) {
            return Ok(f);
        }
    }
    // Day 7: base64 / percent / hex, material reversed first when the Decode body reverses.
    let mut m = material.clone();
    if re!(r"index--|\$\(\s*rev\s*\)|\|\s*rev\b").is_match(&body) {
        m = m.chars().rev().collect();
    }
    let data = if body.contains("base64") {
        crate::codec::base64(&m)
    } else if body.contains("//%/") || re!(r"(?:%[0-9a-fA-F]{2})+").fullmatch(&m).is_some() {
        Some(re!(r"%([0-9a-fA-F]{2})").findall(&m).iter().map(|h| u8::from_str_radix(h, 16).unwrap()).collect())
    } else if body.contains("xxd") || body.contains("\\x%s") {
        crate::codec::hex(&re!(r"[^0-9a-fA-F]").sub(&m, ""))
    } else if re!(r"\d+(?:,\d+)*").fullmatch(&m).is_some() {
        Some(m.split(',').filter_map(|x| x.parse::<u16>().ok()).map(|x| x as u8).collect())
    } else if re!(r"(?:[0-9a-fA-F]{2})+").fullmatch(&m).is_some() {
        crate::codec::hex(&m)
    } else {
        None
    };
    if let Some(d) = data {
        let f = fields(&String::from_utf8_lossy(&d));
        if valid(&f) {
            return Ok(f);
        }
    }
    // Last resort: the Codec_ label, validated.
    let label = labelled::decode_with(text, |s| s.starts_with("v=") && s.contains("feature="))?;
    Ok(fields(&label.descriptor))
}

pub fn answer_adv(key: &str, f: &BTreeMap<String, String>) -> Result<String, String> {
    let get = |k: &str| f.get(k).cloned().ok_or_else(|| format!("{key}: no {k}"));
    if key.ends_with("-role") {
        return Ok("advanced_syntax".into());
    }
    if key.ends_with("-attack") {
        return Ok("T1059.004".into());
    }
    if key.ends_with("-feature") {
        return get("feature");
    }
    if key.ends_with("-primitive") {
        return get("primitive");
    }
    if key.ends_with("-campaign") {
        return get("campaign_id");
    }
    if key.ends_with("-stage-count") {
        return Ok(get("stage_count")?.trim().parse::<i64>().map_err(|e| e.to_string())?.to_string());
    }
    if key.ends_with("-steps") {
        return Ok(get("steps")?.split(',').collect::<Vec<_>>().join(">"));
    }
    Err(format!("unrouted advanced bash question {key}"))
}

// ---- Polyglot / host descriptor literals (v=3) ----

pub const ML_PREFIXES: &[&str] = &["polyglot-", "host-hta-", "host-wsf-", "host-php-", "host-sct-", "host-lua-"];

/// Finds and decodes the descriptor literal: `descriptor_x = "..."` (any language's assignment
/// syntax), MOF `Descriptor = "...";`, then - for languages not seen yet - the
/// `HUNTRESS_ANALYSIS_V1:<KIND>-<payload>` marker or any string that decodes to a descriptor.
/// The assignment may sit inside an escaped string (Day 19 `.aspx` host: `descriptor_x = \"...\"`).
pub fn analyse_literal(text: &str) -> Result<BTreeMap<String, String>, String> {
    let ok = |f: &BTreeMap<String, String>| f.contains_key("role") || f.contains_key("steps");
    for m in [re!(r#"(?i)descriptor_\w+\s*(?::\s*\w+\s*)?(?::=|[=:])\s*\\?"([^"\\]+)\\?""#).search(text), re!(r#"\bDescriptor\s*=\s*"([^"]+)"\s*;"#).search(text)].into_iter().flatten() {
        if let Ok(f) = decode_literal(m.g(1)) {
            if ok(&f) {
                return Ok(f);
            }
        }
    }
    if let Some(m) = re!(r"HUNTRESS_ANALYSIS_V1:[A-Za-z0-9_]+-([A-Za-z0-9+/=.]{8,})").search(text) {
        if let Ok(f) = decode_literal(m.g(1)) {
            if ok(&f) {
                return Ok(f);
            }
        }
    }
    let mut found: Vec<BTreeMap<String, String>> = Vec::new();
    // Day 18 (Apps Script): the literal is split over a list of quoted fragments (`[ "..", ".." ].join("")`,
    // or `+` concatenation), the marker copy's first fragment carrying a `KIND-` prefix. Join each run.
    for m in re!(r#"(?:"[A-Za-z0-9+/=._-]*"\s*[,+]\s*)+"[A-Za-z0-9+/=._-]*""#).find_iter(text) {
        let joined: String = re!(r#""([^"]*)""#).findall(m.g(0)).concat();
        let body = re!(r"^[A-Za-z0-9_]+-").sub(&joined, "");
        if let Ok(f) = decode_literal(&body) {
            if ok(&f) && f.contains_key("steps") && !found.contains(&f) {
                found.push(f);
            }
        }
    }
    if found.len() == 1 {
        return Ok(found.pop().unwrap());
    }
    found.clear();
    for m in re!(r#"["'>]([A-Za-z0-9+/=.]{16,})["'<]"#).find_iter(text) {
        if let Ok(f) = decode_literal(m.g(1)) {
            if ok(&f) && !found.contains(&f) {
                found.push(f);
            }
        }
    }
    match found.len() {
        1 => Ok(found.pop().unwrap()),
        0 => Err("no descriptor literal".into()),
        n => Err(format!("{n} different descriptor literals")),
    }
}

pub fn answer_literal(key: &str, d: &BTreeMap<String, String>) -> Result<String, String> {
    let get = |k: &str| d.get(k).cloned().ok_or_else(|| format!("{key}: no {k} in {:?}", d.keys().collect::<Vec<_>>()));
    if key.ends_with("-role") {
        return get("role");
    }
    if key.ends_with("-attack") {
        return get("attack");
    }
    if key.ends_with("-steps") {
        return Ok(get("steps")?.split(',').collect::<Vec<_>>().join(">"));
    }
    if key.ends_with("-stage-count") {
        return Ok(get("stage_count")?.trim().parse::<i64>().map_err(|e| e.to_string())?.to_string());
    }
    let keymap: &[(&str, &[&str])] = &[
        ("campaign", &["campaign_id"]),
        ("obfuscation", &["obfuscation_profile"]),
        ("host-profile", &["host_profile"]),
        ("stage-profile", &["stage_profile"]),
        ("next-language", &["next_language"]),
    ];
    for suf in ["obfuscation", "host-profile", "stage-profile", "next-language", "campaign", "adapter", "url", "file-path"] {
        if key.ends_with(&format!("-{suf}")) {
            let extra = keymap.iter().find(|(s, _)| *s == suf).map(|(_, k)| *k).unwrap_or(&[]);
            let own = suf.replace('-', "_");
            for k in extra.iter().copied().chain(std::iter::once(own.as_str())) {
                if let Some(v) = d.get(k) {
                    return Ok(v.clone());
                }
            }
            return Err(format!("{key}: no key {suf} in {:?}", d.keys().collect::<Vec<_>>()));
        }
    }
    Err(format!("unrouted {key}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Day 17 AutoHotkey `descriptor_x := "..."` must match the assignment pattern itself (the
    /// first pass only found it via the `HUNTRESS_ANALYSIS_V1` marker), as must Ruby's uppercase constant.
    #[test]
    fn literal_without_marker() {
        let text = std::fs::read_to_string("testdata/d17-polyglot-ahk-downloader.ahk").unwrap();
        let want = analyse_literal(&text).unwrap();
        // A second (unassigned) literal makes the quoted-token fallback ambiguous.
        let decoy = std::fs::read_to_string("testdata/d17-polyglot-ahk-decoy.txt").unwrap();
        let stripped: String = text.lines().filter(|l| !l.contains("HUNTRESS_ANALYSIS_V1")).map(|l| format!("{l}\n")).collect::<String>() + &decoy;
        assert!(analyse_literal(&decoy).unwrap() != want);
        assert_eq!(analyse_literal(&stripped).unwrap(), want);
        let ruby = stripped.replace("descriptor_9d0cb65a :=", "DESCRIPTOR_9D0CB65A =");
        assert_eq!(analyse_literal(&ruby).unwrap(), want);
    }

    /// Day 18 Apps Script splits the literal over `[ "..", ".." ].join("")` (marker copy prefixed `B64-`);
    /// a lone fragment decodes to a truncated descriptor that the token fallback used to accept.
    #[test]
    fn split_fragment_literal() {
        let text = std::fs::read_to_string("testdata/d18-polyglot-gas-staged-chain.gs").unwrap();
        let f = analyse_literal(&text).unwrap();
        assert_eq!(f.get("steps").map(String::as_str), Some("acquire_remote,dispatch_process"));
        assert_eq!(f.get("file_path").map(String::as_str), Some("/tmp/huntress-google_apps_script/.58db6ebfbb172c43.stage"));
        let no_marker: String = text.lines().skip_while(|l| !l.starts_with("function")).map(|l| format!("{l}\n")).collect();
        assert_eq!(analyse_literal(&no_marker).unwrap(), f);
    }

    /// Day 19 `.aspx` host embeds the C# script as one escaped string (`descriptor_x = \"..\"`); the
    /// first pass only found it via the `HEX-` marker.
    #[test]
    fn escaped_literal_without_marker() {
        let text = std::fs::read_to_string("testdata/d19-host-aspx-csharp.csx").unwrap();
        let want = analyse_literal(&text).unwrap();
        assert_eq!(want.get("host_profile").map(String::as_str), Some("aspx_inline_csharp"));
        assert_eq!(analyse_literal(&text.replace("HUNTRESS_ANALYSIS_V1:", "")).unwrap(), want);
    }
}
