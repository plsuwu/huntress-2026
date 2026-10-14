//! The per-field oracle: on an *untimed* attempt, a submit whose answers are all correct is
//! banked (`accepted` + `remaining`), while any wrong answer makes the whole submit `incorrect`.
//! Submitting chunks and bisecting the rejected ones isolates exactly which answers are wrong.
//!
//! `ctf-run oracle <run-dir> [--chunk N]`: takes the challenge from `meta.json` and the answers from
//! `answers.json`; writes `oracle.json` (accepted / rejected ids) into the run dir.

use crate::gm::Gm;
use crate::json::{Value, obj};
use crate::log::ms;
use crate::model::Answers;
use crate::net::pool::Pool;
use crate::say;
use std::path::Path;

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub fn main(dir: &Path, pool: &Pool, chunk: usize) -> Res<bool> {
    let read = |name: &str| -> Res<Value> {
        let p = dir.join(name);
        Ok(Value::parse_bytes(&std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?)?)
    };
    let meta = read("meta.json")?;
    let challenge = meta.str_of("challenge").ok_or("meta.json has no challenge")?.to_string();
    let answers = read("answers.json")?;
    let answers = Answers::from_value(answers.get("answers").unwrap_or(&answers)).ok_or("answers.json is not an id->string map")?;
    let gm = Gm { pool };

    let (status, _) = gm.status(&challenge)?;
    if status.get("deadline").is_some_and(|d| !d.is_null()) {
        return Err("the current attempt is timed: the oracle only runs on untimed attempts".into());
    }
    let already: Vec<String> = status
        .get("accepted_ids")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    let todo: Vec<(String, String)> =
        answers.iter().filter(|(k, _)| !already.iter().any(|a| a == k)).map(|(k, v)| (k.to_string(), v.to_string())).collect();
    say!("oracle", "{challenge}: {} answers, {} already accepted, {} to test", answers.len(), already.len(), todo.len());

    let mut accepted: Vec<String> = Vec::new();
    let mut rejected: Vec<(String, String)> = Vec::new();
    let mut stack: Vec<Vec<(String, String)>> = todo.chunks(chunk.max(1)).rev().map(|c| c.to_vec()).collect();
    let mut solved = false;
    while let Some(group) = stack.pop() {
        let mut a = Answers::new();
        for (k, v) in &group {
            a.set(k, v);
        }
        let (r, resp) = gm.submit(&challenge, &a)?;
        say!("oracle", "{} answer(s) -> {} remaining={:?} ({:.0} ms)", group.len(), r.result, r.remaining, ms(resp.total));
        match r.result.as_str() {
            "CORRECT" => {
                accepted.extend(group.iter().map(|(k, _)| k.clone()));
                solved = true;
                say!("oracle", "CORRECT: flag {}", r.flag.as_deref().unwrap_or("-"));
            }
            "accepted" if r.accepted_ids.is_empty() => accepted.extend(group.into_iter().map(|(k, _)| k)),
            "accepted" => {
                // Trust only what the server lists; anything it left out is re-tested on its own
                // (a mixed submit may bank the correct part).
                let (ok, rest): (Vec<_>, Vec<_>) = group.into_iter().partition(|(k, _)| r.accepted_ids.iter().any(|a| a == k));
                accepted.extend(ok.into_iter().map(|(k, _)| k));
                if rest.len() == 1 && !r.accepted_ids.is_empty() {
                    rejected.push(rest.into_iter().next().unwrap());
                } else {
                    stack.extend(rest.into_iter().rev().map(|x| vec![x]));
                }
            }
            "incorrect" if group.len() == 1 => rejected.push(group[0].clone()),
            "incorrect" => {
                let mid = group.len() / 2;
                stack.push(group[mid..].to_vec());
                stack.push(group[..mid].to_vec());
            }
            other => return Err(format!("unexpected submit result {other:?}: {}", crate::gm::trim(&r.raw)).into()),
        }
    }
    for (id, v) in &rejected {
        say!("rejected", "{id} = {v:?}");
    }
    let out = obj([
        ("challenge", challenge.as_str().into()),
        ("accepted", Value::Arr(accepted.iter().map(|s| s.as_str().into()).collect())),
        ("rejected", Value::Obj(rejected.iter().map(|(k, v)| (k.clone(), Value::Str(v.clone()))).collect())),
        ("solved", solved.into()),
    ]);
    std::fs::write(dir.join("oracle.json"), out.pretty())?;
    say!("oracle", "{} accepted, {} rejected; written {}", accepted.len(), rejected.len(), dir.join("oracle.json").display());
    Ok(rejected.is_empty())
}
