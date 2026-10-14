//! The Game Master protocol: request bodies, response normalisation, route metadata.
//!
//! The proxy and the external API speak the same JSON (`op` fields included); they differ only
//! in URL and authentication, which live in [`crate::net::http::Endpoint`].

use crate::json::{self, Value};
use crate::model::{Answers, DeliverySpec, Mode, Question};
use crate::net::http::Response;
use crate::net::pool::Pool;
use std::error::Error;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub const PROXY_URL: &str = "http://10.0.0.219/huntress-ctf-proxy";
pub const EXTERNAL_URL: &str = "https://2026.huntress.ctf.games";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartOp {
    /// Start, or reconnect to the current attempt.
    Start,
    /// Advance to the next variant after an expired or completed attempt.
    Retry,
    /// Legacy untimed continuation of an exhausted timed route.
    Practice,
}

impl StartOp {
    pub fn name(self) -> &'static str {
        match self {
            StartOp::Start => "start",
            StartOp::Retry => "retry",
            StartOp::Practice => "practice",
        }
    }
}

/// A normalised start/retry/practice response.
pub struct Attempt {
    pub challenge: String,
    pub mode: Mode,
    pub attempt_id: Option<String>,
    pub started_at: Option<String>,
    pub deadline: Option<String>,
    pub run_for_time: Option<bool>,
    pub hall_of_fame_eligible: Option<bool>,
    pub questions: Vec<Question>,
    pub answer_ids: Vec<String>,
    pub accepted_ids: Vec<String>,
    pub deliveries: Vec<DeliverySpec>,
    /// How the downloads were offered, e.g. `artifacts[5]`, `download`, `bundle-preferred`.
    pub shape: String,
    pub raw: Value,
}

/// The 64-hex capability in a download link (`/?op=download&capability=<hex>` or `/api/.../download/<hex>`).
pub fn capability(link: &str) -> Option<&str> {
    link.split(|c: char| !c.is_ascii_hexdigit()).find(|run| run.len() == 64)
}

pub fn download_query(spec: &DeliverySpec) -> String {
    format!("op=download&capability={}", spec.cap)
}

impl Attempt {
    pub fn from_value(challenge: &str, raw: Value) -> Result<Attempt> {
        let questions: Vec<Question> = raw
            .get("questions")
            .and_then(Value::as_array)
            .map(|qs| qs.iter().filter_map(Question::from_json).collect())
            .unwrap_or_default();
        let strs = |key: &str| -> Vec<String> {
            raw.get(key)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default()
        };
        let mut answer_ids = strs("answer_ids");
        if answer_ids.is_empty() {
            answer_ids = questions.iter().map(|q| q.id.clone()).collect();
        }
        let (deliveries, shape) = normalise_deliveries(&raw)?;
        let s = |k: &str| raw.str_of(k).map(str::to_string);
        Ok(Attempt {
            challenge: challenge.to_string(),
            mode: Mode::split(challenge).1,
            attempt_id: s("attempt_id"),
            started_at: s("started_at"),
            deadline: s("deadline"),
            run_for_time: raw.get("run_for_time").and_then(Value::as_bool),
            hall_of_fame_eligible: raw.get("hall_of_fame_eligible").and_then(Value::as_bool),
            accepted_ids: strs("accepted_ids"),
            questions,
            answer_ids,
            deliveries,
            shape,
            raw,
        })
    }

    pub fn is_timed(&self) -> bool {
        self.deadline.is_some()
    }

    /// A `start` on an already-solved attempt carries no downloads; `status` does. Takes the
    /// deliveries (and attempt metadata) from a status response.
    pub fn adopt_status(&mut self, status: &Value) -> Result<()> {
        let (deliveries, shape) = normalise_deliveries(status)?;
        self.deliveries = deliveries;
        self.shape = format!("{shape} (from status)");
        let s = |k: &str| status.str_of(k).map(str::to_string);
        self.attempt_id = self.attempt_id.take().or_else(|| s("attempt_id"));
        self.started_at = self.started_at.take().or_else(|| s("started_at"));
        self.deadline = self.deadline.take().or_else(|| s("deadline"));
        Ok(())
    }
}

/// Collects every download the response offers, de-duplicated by capability.
///
/// Shapes seen: `artifacts[] {download, filename, sha256, size?, kind?}` with a parallel
/// `downloads[]`; a lone top-level `download` (+ `sha256`); a regular mastery's single
/// `kind:"bundle"` artifact. When a bundle is offered next to per-file downloads, only the bundle
/// is kept: one request instead of N, each of which risks the server's 2-8 s tail.
fn normalise_deliveries(raw: &Value) -> Result<(Vec<DeliverySpec>, String)> {
    let mut specs: Vec<DeliverySpec> = Vec::new();
    fn push(specs: &mut Vec<DeliverySpec>, link: &str, filename: Option<&str>, sha: Option<&str>, kind: Option<&str>, size: Option<u64>) {
        let Some(cap) = capability(link) else { return };
        if specs.iter().any(|s| s.cap == cap) {
            return;
        }
        specs.push(DeliverySpec {
            index: specs.len(),
            link: link.to_string(),
            cap: cap.to_string(),
            filename: filename.map(str::to_string),
            sha256: sha.map(str::to_string),
            kind: kind.map(str::to_string),
            size,
        });
    }
    let mut parts = Vec::new();
    if let Some(arts) = raw.get("artifacts").and_then(Value::as_array) {
        for a in arts {
            if let Some(link) = a.str_of("download") {
                push(
                    &mut specs,
                    link,
                    a.str_of("filename").or_else(|| a.str_of("artifact")),
                    a.str_of("sha256"),
                    a.str_of("kind"),
                    a.get("size").and_then(Value::as_i64).map(|n| n as u64),
                );
            }
        }
        parts.push(format!("artifacts[{}]", arts.len()));
    }
    if let Some(dls) = raw.get("downloads").and_then(Value::as_array) {
        for d in dls {
            if let Some(link) = d.as_str() {
                push(&mut specs, link, None, None, None, None);
            }
        }
        parts.push(format!("downloads[{}]", dls.len()));
    }
    if let Some(link) = raw.str_of("download") {
        let before = specs.len();
        push(&mut specs, link, raw.str_of("filename"), raw.str_of("sha256"), None, None);
        // A top-level download next to a per-file list is the bundle.
        if before > 0 && specs.len() > before {
            specs.last_mut().unwrap().kind = Some("bundle".into());
        }
        parts.push("download".into());
    }
    let mut shape = parts.join("+");
    let bundles: Vec<DeliverySpec> = specs.iter().filter(|s| s.kind.as_deref() == Some("bundle")).cloned().collect();
    if !bundles.is_empty() && bundles.len() < specs.len() {
        specs = bundles;
        for (i, s) in specs.iter_mut().enumerate() {
            s.index = i;
        }
        shape.push_str(" -> bundle-preferred");
    }
    Ok((specs, shape))
}

#[derive(Debug)]
pub struct SubmitResult {
    /// `CORRECT`, `accepted`, `incorrect`, `expired`, ...
    pub result: String,
    pub remaining: Option<i64>,
    pub accepted_ids: Vec<String>,
    pub flag: Option<String>,
    pub raw: Value,
}

impl SubmitResult {
    pub fn from_value(raw: Value) -> SubmitResult {
        SubmitResult {
            result: raw.str_of("result").unwrap_or("").to_string(),
            remaining: raw.get("remaining").and_then(Value::as_i64),
            accepted_ids: raw
                .get("accepted_ids")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
            flag: raw.str_of("flag").map(str::to_string),
            raw,
        }
    }

    pub fn correct(&self) -> bool {
        self.result == "CORRECT"
    }
}

/// Request bodies and calls.
pub struct Gm<'p> {
    pub pool: &'p Pool,
}

fn json_body(resp: &Response) -> Result<Value> {
    Value::parse_bytes(&resp.body).map_err(|e| {
        format!("HTTP {}: {e}; body starts {:?}", resp.status, String::from_utf8_lossy(&resp.body[..resp.body.len().min(200)])).into()
    })
}

impl Gm<'_> {
    pub fn start_body(challenge: &str, op: StartOp, run_for_time: bool) -> String {
        let mut b = String::from("{\"op\":");
        json::write_str(&mut b, op.name());
        b.push_str(",\"challenge\":");
        json::write_str(&mut b, challenge);
        if run_for_time {
            b.push_str(",\"run_for_time\":true");
        }
        b.push('}');
        b
    }

    pub fn submit_body(challenge: &str, answers: &Answers) -> String {
        let mut b = String::with_capacity(64 + answers.len() * 48);
        b.push_str("{\"op\":\"submit\",\"challenge\":");
        json::write_str(&mut b, challenge);
        b.push_str(",\"answers\":");
        answers.to_value().write(&mut b);
        b.push('}');
        b
    }

    /// `start` / `retry` / `practice`. Returns the raw response too (for timings and saving).
    pub fn start(&self, challenge: &str, op: StartOp, run_for_time: bool) -> Result<(Attempt, Response)> {
        let resp = self.pool.post_json(&Self::start_body(challenge, op, run_for_time))?;
        let v = json_body(&resp)?;
        if !resp.ok() || v.get("questions").is_none() {
            return Err(format!("{} failed: HTTP {} {}", op.name(), resp.status, trim(&v)).into());
        }
        Ok((Attempt::from_value(challenge, v)?, resp))
    }

    pub fn submit(&self, challenge: &str, answers: &Answers) -> Result<(SubmitResult, Response)> {
        let resp = self.pool.post_json(&Self::submit_body(challenge, answers))?;
        let v = json_body(&resp)?;
        Ok((SubmitResult::from_value(v), resp))
    }

    pub fn status(&self, challenge: &str) -> Result<(Value, Response)> {
        let resp = self.pool.get_query(&format!("op=status&challenge={}", query_escape(challenge)))?;
        Ok((json_body(&resp)?, resp))
    }

    pub fn list(&self) -> Result<(Value, Response)> {
        let resp = self.pool.get_query("op=list&all=true")?;
        Ok((json_body(&resp)?, resp))
    }
}

/// A response without its bulky fields, for logs.
pub fn trim(v: &Value) -> String {
    match v {
        Value::Obj(m) => Value::Obj(
            m.iter()
                .filter(|(k, _)| !matches!(k.as_str(), "guidance" | "questions" | "prompt" | "answer_ids" | "accepted_ids"))
                .cloned()
                .collect(),
        )
        .to_json(),
        _ => v.to_json(),
    }
}

/// Escapes a challenge id for a query string (ids are `[a-z0-9-/_]`, so only `/` matters, and the
/// Game Master accepts it raw; anything unexpected is percent-encoded).
pub fn query_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_./".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Route metadata from `op=list` (cached on disk: the list call takes 8-24 s through the proxy).
#[derive(Clone, Debug)]
pub struct Route {
    pub id: String,
    pub run_for_time_supported: bool,
    pub ranked_deadline_ms: Option<i64>,
    pub ranked_pool_size: Option<i64>,
    pub regular_pool_size: Option<i64>,
    pub ranked_set_count: Option<i64>,
    pub regular_set_count: Option<i64>,
    pub solved: bool,
}

impl Route {
    pub fn from_value(v: &Value) -> Option<Route> {
        let n = |k: &str| v.get(k).and_then(Value::as_i64);
        Some(Route {
            id: v.str_of("id")?.to_string(),
            run_for_time_supported: v.get("run_for_time_supported").and_then(Value::as_bool).unwrap_or(false),
            ranked_deadline_ms: n("ranked_deadline_ms"),
            ranked_pool_size: n("ranked_pool_size"),
            regular_pool_size: n("regular_pool_size"),
            ranked_set_count: n("ranked_set_count"),
            regular_set_count: n("regular_set_count"),
            solved: v.get("solved").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    /// A timed-by-default route with a single ranked attempt and no regular pool.
    pub fn one_shot(&self) -> bool {
        !self.run_for_time_supported && self.ranked_pool_size == Some(1)
    }
}

pub fn routes_cache_path() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("ctf-run").join("routes.json")
}

/// Routes from the cache file, if present.
pub fn cached_routes() -> Option<Vec<Route>> {
    let text = std::fs::read(routes_cache_path()).ok()?;
    routes_from_list(&Value::parse_bytes(&text).ok()?)
}

pub fn routes_from_list(list: &Value) -> Option<Vec<Route>> {
    Some(list.get("challenges")?.as_array()?.iter().filter_map(Route::from_value).collect())
}

pub fn save_routes(list: &Value) -> std::io::Result<PathBuf> {
    let p = routes_cache_path();
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, list.to_json())?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(c: char) -> String {
        c.to_string().repeat(64)
    }

    #[test]
    fn shapes() {
        // Per-set files: artifacts[] + parallel downloads[].
        let v = Value::parse(&format!(
            r#"{{"questions":[],"downloads":["/api/x/download/{a}","/api/x/download/{b}"],
               "artifacts":[{{"download":"/api/x/download/{a}","filename":"x_01.apk","sha256":"s1"}},
                            {{"download":"/api/x/download/{b}","filename":"x_02.apk"}}]}}"#,
            a = cap('a'),
            b = cap('b')
        ))
        .unwrap();
        let (d, shape) = normalise_deliveries(&v).unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].filename.as_deref(), d[0].sha256.as_deref()), (Some("x_01.apk"), Some("s1")));
        assert_eq!(shape, "artifacts[2]+downloads[2]");

        // Lone top-level bundle.
        let v = Value::parse(&format!(r#"{{"questions":[],"download":"/?op=download&capability={}","sha256":"z"}}"#, cap('c'))).unwrap();
        let (d, shape) = normalise_deliveries(&v).unwrap();
        assert_eq!((d.len(), d[0].cap.as_str(), shape.as_str()), (1, cap('c').as_str(), "download"));

        // Bundle next to per-file downloads: keep the bundle only.
        let v = Value::parse(&format!(
            r#"{{"questions":[],"downloads":["/d/{a}","/d/{b}"],"download":"/d/{c}"}}"#,
            a = cap('a'),
            b = cap('b'),
            c = cap('c')
        ))
        .unwrap();
        let (d, shape) = normalise_deliveries(&v).unwrap();
        assert_eq!((d.len(), d[0].cap.as_str(), d[0].index), (1, cap('c').as_str(), 0));
        assert!(shape.ends_with("bundle-preferred"), "{shape}");
    }

    #[test]
    fn bodies() {
        assert_eq!(
            Gm::start_body("w-day-13/mastery", StartOp::Retry, true),
            r#"{"op":"retry","challenge":"w-day-13/mastery","run_for_time":true}"#
        );
        let mut a = Answers::new();
        a.set("set1.message", "a\"b");
        assert_eq!(Gm::submit_body("c", &a), r#"{"op":"submit","challenge":"c","answers":{"set1.message":"a\"b"}}"#);
        assert_eq!(capability(&format!("/api/huntress/game-master/download/{}", cap('d'))), Some(cap('d').as_str()));
        assert_eq!(query_escape("a-day-1/mastery"), "a-day-1/mastery");
    }
}
