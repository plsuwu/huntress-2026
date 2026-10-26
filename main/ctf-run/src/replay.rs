//! Offline replay: run a lane over saved material and diff against expected answers.
//!
//! Inputs (any mix, directories are searched recursively for `*.json` manifests and run dirs):
//!   - a saved run directory (`start.json`, `meta.json`, `deliveries/NN-name`, `answers.json`);
//!   - a fixture manifest (`fixtures/<lane>/<case>.json`):
//!     `{"challenge": "...", "start": "<path>" | {..start json..}, "deliveries": ["<path>" | {"path","name"}],
//!       "expected": "<path>" | {id: value}, "graded": "correct" | "solver", "note": "..."}`
//!     Paths are relative to the manifest's directory. An expected file may be `{"answers": {...}}` or a flat object.

use crate::gm::Attempt;
use crate::json::Value;
use crate::lanes::{self, Analysed, Done};
use crate::log::ms;
use crate::model::{Answers, Delivery, DeliverySpec, RunCtx};
use crate::run::{Answerer, saved_deliveries};
use crate::say;
use std::path::{Path, PathBuf};
use std::time::Instant;

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct Case {
    pub label: String,
    pub challenge: String,
    pub start: Value,
    /// (name, bytes) in delivery order.
    pub deliveries: Vec<(String, Vec<u8>)>,
    pub expected: Option<Answers>,
    pub graded: String,
}

fn read_json(p: &Path) -> Res<Value> {
    let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(Value::parse_bytes(&bytes).map_err(|e| format!("{}: {e}", p.display()))?)
}

fn answers_from(v: &Value) -> Option<Answers> {
    Answers::from_value(v.get("answers").unwrap_or(v))
}

/// Loads a saved run directory.
pub fn load_run_dir(dir: &Path) -> Res<Case> {
    let start = read_json(&dir.join("start.json"))?;
    let meta = read_json(&dir.join("meta.json")).ok();
    let challenge = meta
        .as_ref()
        .and_then(|m| m.str_of("challenge"))
        .map(str::to_string)
        .ok_or("meta.json with a challenge is required")?;
    let mut deliveries = Vec::new();
    for p in saved_deliveries(dir)? {
        let file = p.file_name().unwrap().to_string_lossy().into_owned();
        let name = match file.split_once('-') {
            Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) => rest.to_string(),
            _ => file,
        };
        deliveries.push((name, std::fs::read(&p)?));
    }
    let expected = read_json(&dir.join("expected.json")).ok().as_ref().and_then(answers_from);
    Ok(Case {
        label: dir.display().to_string(),
        challenge,
        start,
        deliveries,
        expected,
        graded: "run".into(),
    })
}

/// Loads a fixture manifest.
pub fn load_manifest(path: &Path) -> Res<Case> {
    let m = read_json(path)?;
    let base = path.parent().unwrap_or(Path::new("."));
    let resolve = |s: &str| -> PathBuf { base.join(s) };
    let challenge = m.str_of("challenge").ok_or("manifest needs a challenge")?.to_string();
    let start = match m.get("start") {
        Some(Value::Str(p)) => read_json(&resolve(p))?,
        Some(v @ Value::Obj(_)) => v.clone(),
        _ => return Err("manifest needs start".into()),
    };
    let mut deliveries = Vec::new();
    for d in m.get("deliveries").and_then(Value::as_array).unwrap_or(&[]) {
        let (p, name) = match d {
            Value::Str(p) => (p.as_str(), None),
            _ => (d.str_of("path").ok_or("delivery needs a path")?, d.str_of("name")),
        };
        let full = resolve(p);
        let bytes = std::fs::read(&full).map_err(|e| format!("{}: {e}", full.display()))?;
        let name = name.map(str::to_string).unwrap_or_else(|| full.file_name().unwrap().to_string_lossy().into_owned());
        deliveries.push((name, bytes));
    }
    let expected = match m.get("expected") {
        Some(Value::Str(p)) => answers_from(&read_json(&resolve(p))?),
        Some(v @ Value::Obj(_)) => answers_from(v),
        _ => None,
    };
    Ok(Case {
        label: path.display().to_string(),
        challenge,
        start,
        deliveries,
        expected,
        graded: m.str_of("graded").unwrap_or("unknown").to_string(),
    })
}

/// Expands paths into cases: manifests, run dirs, and directories containing either.
pub fn collect(paths: &[PathBuf]) -> Vec<Res<Case>> {
    let mut out = Vec::new();
    for p in paths {
        walk(p, &mut out);
    }
    out
}

fn walk(p: &Path, out: &mut Vec<Res<Case>>) {
    if p.is_dir() {
        if p.join("start.json").is_file() && p.join("meta.json").is_file() {
            out.push(load_run_dir(p));
            return;
        }
        let mut entries: Vec<PathBuf> = match std::fs::read_dir(p) {
            Ok(rd) => rd.filter_map(|e| Some(e.ok()?.path())).collect(),
            Err(e) => {
                out.push(Err(format!("{}: {e}", p.display()).into()));
                return;
            }
        };
        entries.sort();
        for e in entries {
            if e.is_dir() || e.extension().is_some_and(|x| x == "json") {
                walk(&e, out);
            }
        }
    } else {
        out.push(load_manifest(p));
    }
}

pub struct Outcome {
    pub answers: Answers,
    pub errors: Vec<(String, String)>,
    pub mismatches: Vec<(String, String, String)>,
    pub missing_expected: usize,
}

/// Runs one case through its lane (analyses in parallel, like a live run). The delivery bytes move out of
/// `case` (no second copy), so peak memory matches a live run.
pub fn replay(case: &mut Case, lane_name: Option<&str>) -> Res<Outcome> {
    let mut ctx = RunCtx::new(&case.challenge);
    let lane = lanes::find(lane_name, &ctx.base).ok_or_else(|| format!("no lane for {:?}", ctx.base))?;
    let attempt = Attempt::from_value(&case.challenge, case.start.clone())?;
    ctx.started_ms = attempt.started_at.as_deref().and_then(crate::time::parse_iso);
    let deliveries: Vec<Delivery> = std::mem::take(&mut case.deliveries)
        .into_iter()
        .enumerate()
        .map(|(i, (name, bytes))| Delivery {
            spec: attempt.deliveries.get(i).cloned().unwrap_or_else(|| DeliverySpec {
                index: i,
                link: String::new(),
                cap: String::new(),
                filename: Some(name.clone()),
                sha256: None,
                kind: None,
                size: None,
            }),
            name: name.clone(),
            bytes,
        })
        .collect();
    let mut done = Analysed::new(deliveries.len());
    let t_analyse = Instant::now();
    let analyses: Vec<_> = std::thread::scope(|s| {
        let hs: Vec<_> = deliveries.iter().map(|d| s.spawn(|| lane.analyse(d, &ctx))).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("analysis panicked".into()))).collect()
    });
    let analyse_ms = ms(t_analyse.elapsed());
    let t_answer = Instant::now();
    // Same incremental path as a live run, with deliveries landing in reverse order (the
    // worst case for answers that depend on another delivery).
    let mut answerer = Answerer::new(lane, &ctx, &attempt.questions);
    let mut landed: Vec<(usize, Done)> = deliveries.into_iter().zip(analyses).enumerate().map(|(i, (d, a))| (i, Done { delivery: d, analysis: a })).collect();
    while let Some((i, d)) = landed.pop() {
        done.items[i] = Some(d);
        answerer.on_delivery(i, &done);
    }
    answerer.finish(&done);
    let (answers, errors) = (answerer.answers.clone(), answerer.errors());
    if std::env::var_os("CTF_PROFILE").is_some() {
        say!("profile", "analyse {analyse_ms:.1} ms, answer {:.1} ms", ms(t_answer.elapsed()));
    }
    let mut mismatches = Vec::new();
    let mut missing_expected = 0;
    if let Some(exp) = &case.expected {
        for q in &attempt.questions {
            match (exp.get(&q.id), answers.get(&q.id)) {
                (Some(e), Some(g)) if e != g => mismatches.push((q.id.clone(), e.to_string(), g.to_string())),
                (Some(_), Some(_)) => {}
                (None, _) => missing_expected += 1,
                (Some(e), None) => mismatches.push((q.id.clone(), e.to_string(), "<error>".into())),
            }
        }
    }
    Ok(Outcome { answers, errors, mismatches, missing_expected })
}

/// `ctf-run replay`: returns true when every case answered everything and matched expectations.
pub fn main(paths: &[PathBuf], lane: Option<&str>, show: bool) -> bool {
    let mut all_ok = true;
    let (mut n, mut good) = (0, 0);
    for case in collect(paths) {
        n += 1;
        let mut case = match case {
            Ok(c) => c,
            Err(e) => {
                say!("replay", "LOAD FAILED: {e}");
                all_ok = false;
                continue;
            }
        };
        let t = Instant::now();
        match replay(&mut case, lane) {
            Err(e) => {
                say!("replay", "{}: FAILED: {e}", case.label);
                all_ok = false;
            }
            Ok(o) => {
                let ok = o.errors.is_empty() && o.mismatches.is_empty();
                good += ok as usize;
                all_ok &= ok;
                say!(
                    "replay",
                    "{} {} [{}]: {} answers, {} errors, {} mismatches{} in {:.1} ms",
                    if ok { "OK  " } else { "FAIL" },
                    case.label,
                    case.graded,
                    o.answers.len(),
                    o.errors.len(),
                    o.mismatches.len(),
                    if case.expected.is_none() { " (no expected answers)".to_string() } else if o.missing_expected > 0 { format!(", {} not in expected", o.missing_expected) } else { String::new() },
                    ms(t.elapsed())
                );
                for (id, e) in o.errors.iter().take(20) {
                    say!("  error", "{id}: {e}");
                }
                for (id, want, got) in o.mismatches.iter().take(20) {
                    say!("  diff", "{id}: expected {want:?}, got {got:?}");
                }
                if show {
                    println!("{}", o.answers.to_value().pretty());
                }
            }
        }
    }
    say!("replay", "{good}/{n} case(s) clean");
    all_ok
}
