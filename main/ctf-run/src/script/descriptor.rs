//! The pipe descriptor every script specimen decodes to (`v=1|steps=a,b|adapter=..|stage_count=N|...`),
//! the step-graph -> role / ATT&CK rubric, and the question-suffix -> descriptor-key tables.

use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub steps: Vec<String>,
    pub adapter: String,
    pub stage_count: Option<i64>,
    pub graph: Option<String>,
    /// Every other `key=value` field.
    pub values: BTreeMap<String, String>,
}

/// `parse_pipe_descriptor`: requires `v`, `steps` and `adapter`.
pub fn parse_pipe(text: &str) -> Result<Plan, String> {
    let mut plan = Plan::default();
    let mut seen_v = false;
    for part in text.trim().split('|') {
        let Some((k, v)) = part.split_once('=') else { continue };
        match k {
            "v" => seen_v = true,
            "steps" => plan.steps = v.split(',').map(str::to_string).collect(),
            "adapter" => plan.adapter = v.to_string(),
            "stage_count" => plan.stage_count = Some(v.parse().map_err(|_| format!("bad stage_count {v:?}"))?),
            "graph" => plan.graph = Some(v.to_string()),
            _ => {
                plan.values.insert(k.to_string(), v.to_string());
            }
        }
    }
    if !(seen_v && !plan.steps.is_empty() && !plan.steps[0].is_empty() && !plan.adapter.is_empty()) {
        return Err(format!("not a pipe descriptor: {:?}", text.chars().take(60).collect::<String>()));
    }
    Ok(plan)
}

/// `key=value` fields of a `|`-separated record (no validation).
pub fn fields(raw: &str) -> BTreeMap<String, String> {
    raw.split('|').map(|p| p.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())).unwrap_or((p.to_string(), String::new()))).collect()
}

/// Is `text` a plausible descriptor (used to validate candidate decodes)?
pub fn looks_valid(text: &str) -> bool {
    text.starts_with("v=") && parse_pipe(text).is_ok()
}

pub fn role_of(steps: &[String]) -> Result<&'static str, String> {
    let last = steps.last().ok_or("no steps")?.as_str();
    let has = |s: &str| steps.iter().any(|x| x == s);
    Ok(match last {
        "return_configuration" => "benign_utility",
        "dispatch_next_stage" => "relay",
        "handoff_script" => "staged_chain",
        "upload_remote" | "transmit_remote" => "uploader",
        "write_recovery_note" => "impact",
        "register_descriptor" => "persistence_descriptor",
        "dispatch_memory" | "dispatch_pipe" => "stager",
        "dispatch_process" if has("write_file") => "staged_chain",
        "dispatch_process" if has("locate_local") => "launcher",
        "write_file" if has("acquire_remote") => "downloader",
        "write_file" if has("decode_embedded") => "dropper",
        _ => return Err(format!("unclassified step graph {steps:?}")),
    })
}

/// Primary technique by terminal behaviour; `lang` is the language's T1059 sub-technique.
pub fn attack_of(steps: &[String], lang: &str) -> Result<String, String> {
    Ok(match role_of(steps)? {
        "benign_utility" => "none",
        "relay" | "staged_chain" | "launcher" => lang,
        "uploader" => "T1041",
        "impact" => "T1486",
        "persistence_descriptor" => "T1053.005",
        "stager" | "downloader" => "T1105",
        "dropper" => "T1140",
        _ => unreachable!(),
    }
    .to_string())
}

/// Question suffix -> descriptor keys (Day 3 `VALUE_KEYS`).
pub const VALUE_KEYS: &[(&str, &[&str])] = &[
    ("url", &["url", "upload_url"]),
    ("upload-url", &["upload_url", "url"]),
    ("path", &["file_path", "source_path"]),
    ("file-path", &["file_path", "source_path"]),
    ("next-stage", &["next_stage"]),
    ("host", &["host_id"]),
    ("host-id", &["host_id"]),
    ("note", &["note_name"]),
    ("note-name", &["note_name"]),
    ("extension", &["extension"]),
    ("campaign", &["campaign_id"]),
];

/// Extra keys of the Bash / Python / Node families.
pub const EXTRA_KEYS: &[(&str, &[&str])] = &[
    ("user-agent", &["user_agent"]),
    ("cron-path", &["cron_path"]),
    ("service-name", &["service_name"]),
    ("arch-probe", &["arch_probe"]),
    ("handoff-id", &["handoff_id"]),
    ("schedule-path", &["schedule_path", "cron_path", "task_path"]),
];

/// Looks the question up in `plan.values`: suffixes longest first; `self_key` also tries the
/// suffix itself with `-` -> `_` (the Bash/Python/Node families do, the Day 3 ones do not).
pub fn value_answer(key: &str, plan: &Plan, tables: &[&[(&str, &[&str])]], self_key: bool) -> Option<Result<String, String>> {
    let mut suffixes: Vec<(&str, Vec<&str>)> = Vec::new();
    for t in tables {
        for (s, keys) in t.iter() {
            match suffixes.iter_mut().find(|(x, _)| x == s) {
                Some(e) => e.1 = keys.to_vec(),
                None => suffixes.push((s, keys.to_vec())),
            }
        }
    }
    suffixes.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
    for (s, keys) in suffixes {
        if key.ends_with(&format!("-{s}")) {
            let own = s.replace('-', "_");
            let cands: Vec<&str> = if self_key { std::iter::once(own.as_str()).chain(keys.iter().copied()).collect() } else { keys.clone() };
            for k in cands {
                if let Some(v) = plan.values.get(k) {
                    return Some(Ok(v.clone()));
                }
            }
            return Some(Err(format!("{key}: none of {s} keys in {:?}", plan.values.keys().collect::<Vec<_>>())));
        }
    }
    None
}

/// Polyglot/host literals: layers of hex / base64 / base32 / reversal, `.`-fragmented at any layer
/// (Day 23 `LYR-<encode order>` recipes, e.g. `hex_rev_hex`, `b64_b32_frag`), until `v=`. The layer
/// order is searched (depth 6), not read from the recipe, so a bare literal decodes too.
pub fn decode_literal(lit: &str) -> Result<BTreeMap<String, String>, String> {
    fn go(raw: &[u8], depth: u8, after_rev: bool) -> Option<String> {
        if raw.starts_with(b"v=") {
            return std::str::from_utf8(raw).ok().map(str::to_string);
        }
        let raw: Vec<u8> = raw.iter().copied().filter(|&b| b != b'.').collect();
        if raw.starts_with(b"v=") {
            return std::str::from_utf8(&raw).ok().map(str::to_string);
        }
        if depth == 0 || raw.is_empty() || !raw.is_ascii() {
            return None;
        }
        let s = std::str::from_utf8(&raw).ok()?;
        let hexy = s.len() % 2 == 0 && s.bytes().all(|b| b.is_ascii_hexdigit());
        let mut cands: Vec<Vec<u8>> = Vec::new();
        if hexy {
            cands.extend(crate::codec::hex(s));
        }
        cands.extend(crate::codec::base32(s));
        cands.extend(crate::codec::base64(s));
        for c in cands {
            if let Some(t) = go(&c, depth - 1, false) {
                return Some(t);
            }
        }
        if !after_rev {
            let r: Vec<u8> = raw.iter().rev().copied().collect();
            return go(&r, depth - 1, true);
        }
        None
    }
    go(lit.as_bytes(), 6, false).map(|t| fields(&t)).ok_or_else(|| format!("descriptor did not decode: {:?}", lit.chars().take(40).collect::<String>()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(s: &str) -> Vec<String> {
        s.split(',').map(str::to_string).collect()
    }

    #[test]
    fn rubric() {
        assert_eq!(role_of(&st("acquire_remote,dispatch_memory")).unwrap(), "stager");
        assert_eq!(role_of(&st("profile_host,acquire_remote,write_file,dispatch_process")).unwrap(), "staged_chain");
        assert_eq!(attack_of(&st("decode_embedded,write_file"), "T1059.003").unwrap(), "T1140");
        let p = parse_pipe("v=1|steps=a,locate_local,dispatch_process|adapter=x|stage_count=1|campaign_id=c|host_id=h").unwrap();
        assert_eq!(value_answer("batch-x-host-id", &p, &[VALUE_KEYS], false).unwrap().unwrap(), "h");
        assert_eq!(value_answer("batch-x-campaign", &p, &[VALUE_KEYS], false).unwrap().unwrap(), "c");
        assert!(parse_pipe("v=1|adapter=x").is_err());
        assert_eq!(decode_literal("76.3d.31").unwrap().get("v").map(String::as_str), Some("1"));
    }
}
