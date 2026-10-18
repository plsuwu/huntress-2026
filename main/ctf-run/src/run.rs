//! One live attempt: guard -> preflight -> start -> fetch+analyse (pipelined) -> answer -> submit
//! -> persist.
//!
//! Everything before `start` is free (the server's timer starts there at the earliest) and
//! everything after the final submit is free, so bookkeeping lives at the two ends and the middle
//! only moves bytes and answers.

use crate::gm::{self, Attempt, Gm, Route, StartOp, SubmitResult};
use crate::json::{Value, obj};
use crate::lanes::{self, Analysed, Done, Lane};
use crate::log::ms;
use crate::model::{Answers, Delivery, DeliverySpec, Mode, Question, RunCtx};
use crate::net::fetch;
use crate::net::http::Response;
use crate::net::pool::Pool;
use crate::say;
use crate::time;
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct Opts {
    pub challenge: String,
    pub op: StartOp,
    /// Send `run_for_time:true` (dual-pool routes only).
    pub timed: bool,
    pub dry: bool,
    pub early_submit: bool,
    /// Submit the answers that were found even if some questions failed.
    pub partial: bool,
    pub lane: Option<String>,
    /// Parallel downloads; default one per delivery (capped at [`MAX_JOBS`]).
    pub jobs: Option<usize>,
    pub out: PathBuf,
    pub one_shot_ok: bool,
    pub preflight: bool,
    /// Requests in the warm-up burst fired just before `start` (0 = off).
    pub warm: usize,
    /// Fetch every delivery twice on separate connections and take the first (small deliveries only).
    pub hedge: bool,
}

pub const MAX_JOBS: usize = 64;
const STATUS_WAIT: Duration = Duration::from_millis(2500);

/// Refuses runs that would spend something the caller did not mean to spend. Every `start`
/// (including `--dry`, which still starts or reconnects) goes through here, and it fails closed:
/// an unknown route outside single-solve needs `--one-shot-ok`.
pub fn check_route(mode: Mode, op: StartOp, timed: bool, dry: bool, one_shot_ok: bool, route: Option<&Route>) -> Result<(), String> {
    if mode == Mode::Single {
        return Ok(());
    }
    let Some(r) = route else {
        return if one_shot_ok {
            Ok(())
        } else {
            Err("route not in the cache (`ctf-run routes --refresh`); pass --one-shot-ok to start it blind".into())
        };
    };
    if timed && !r.run_for_time_supported {
        return Err("--timed on a route without run_for_time support (the server answers 409)".into());
    }
    if dry && timed {
        return Err("--dry --timed would start a ranked attempt and never submit it".into());
    }
    let timed_by_default = !r.run_for_time_supported && op != StartOp::Practice;
    // Exception: a plain `start` on a route whose attempt is already solved only reconnects (the material comes
    // from `status`), so it is a free live test of the lane - the hand check before spending ranked attempts.
    if dry && timed_by_default && !(op == StartOp::Start && r.solved) {
        return Err("--dry on a timed-by-default route would start the clock and never submit (unless it is solved: plain start reconnects)".into());
    }
    if timed_by_default && r.one_shot() && !one_shot_ok {
        return Err("one-shot route (timed by default, ranked pool 1): pass --one-shot-ok to spend it".into());
    }
    Ok(())
}

/// `CTF_NO_RANKED=1` (set for unattended lane agents): refuse anything that could spend a ranked attempt -
/// `--timed`, `--one-shot-ok`, and non-dry starts/retries on timed-by-default or uncached routes. Untimed
/// regular play (dual-pool routes), single-solve, practice and dry reconnects stay allowed.
pub fn check_unranked(mode: Mode, op: StartOp, timed: bool, dry: bool, one_shot_ok: bool, route: Option<&Route>) -> Result<(), String> {
    let deny = |why: &str| Err(format!("CTF_NO_RANKED is set: {why} (ranked attempts are the coordinator's call)"));
    if timed {
        return deny("--timed starts a ranked attempt");
    }
    if one_shot_ok {
        return deny("--one-shot-ok spends a one-shot attempt");
    }
    if mode == Mode::Single || op == StartOp::Practice || dry {
        return Ok(()); // dry runs are already limited to safe reconnects by check_route
    }
    match route {
        Some(r) if r.run_for_time_supported => Ok(()),
        Some(_) => deny("this route is timed by default - a start or retry here is ranked"),
        None => deny("route not in the cache, cannot tell whether it is timed"),
    }
}

/// What one delivery worker hands back.
struct Landed {
    index: usize,
    fetch: Result<(Delivery, FetchTiming), String>,
    analysis: Option<Result<lanes::Analysis, String>>,
    analyse_ms: f64,
}

#[derive(Clone, Copy)]
struct FetchTiming {
    ttfb_ms: f64,
    total_ms: f64,
    wire: usize,
    done_at_ms: f64,
}

/// Wall-clock (UTC ms) stamps of the critical path, to line up with the server's
/// `started_at` / `deadline` and the responses' `Date` headers.
#[derive(Default)]
struct Stamps {
    start_sent: i64,
    start_recv: i64,
    start_date: Option<i64>,
    last_delivery: i64,
    submit_sent: i64,
    submit_recv: i64,
    submit_date: Option<i64>,
}

/// A safe single path component from an untrusted name.
pub fn safe_name(name: &str) -> Option<String> {
    let last = name.rsplit(['/', '\\']).next()?;
    let clean: String = last.chars().filter(|c| !c.is_control()).collect();
    let clean = clean.trim();
    (!clean.is_empty() && clean != "." && clean != "..").then(|| clean.to_string())
}

/// File name from a Content-Disposition header.
fn disposition_filename(h: &str) -> Option<String> {
    let mut plain = None;
    for part in h.split(';').map(str::trim) {
        if let Some(v) = part.strip_prefix("filename*=") {
            let enc = v.rsplit('\'').next()?.as_bytes();
            let mut out = Vec::with_capacity(enc.len());
            let mut i = 0;
            while i < enc.len() {
                let hex = (enc[i] == b'%').then(|| enc.get(i + 1..i + 3)).flatten();
                match hex.and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                    }
                    None => {
                        out.push(enc[i]);
                        i += 1;
                    }
                }
            }
            return Some(String::from_utf8_lossy(&out).into_owned());
        }
        if let Some(v) = part.strip_prefix("filename=") {
            plain = Some(v.trim_matches('"').to_string());
        }
    }
    plain
}

pub fn delivery_from(spec: &DeliverySpec, resp: Response) -> Delivery {
    let name = resp
        .header("content-disposition")
        .and_then(disposition_filename)
        .as_deref()
        .and_then(safe_name)
        .or_else(|| spec.filename.as_deref().and_then(safe_name))
        .unwrap_or_else(|| format!("delivery-{}.bin", spec.index + 1));
    Delivery { spec: spec.clone(), name, bytes: resp.body }
}

/// Incremental answering, shared by the live runner and replay: as each delivery lands, the open
/// questions routed to it (`Lane::delivery_of`) are answered; failures stay open and are retried by
/// [`Answerer::finish`] once every delivery is in (an answer may depend on another delivery, e.g.
/// a warmups row transform learned from a different set).
pub struct Answerer<'a> {
    lane: &'a dyn Lane,
    ctx: &'a RunCtx,
    questions: &'a [Question],
    pub answers: Answers,
    answered: HashSet<String>,
    errors: HashMap<String, String>,
}

impl<'a> Answerer<'a> {
    pub fn new(lane: &'a dyn Lane, ctx: &'a RunCtx, questions: &'a [Question]) -> Self {
        Answerer { lane, ctx, questions, answers: Answers::new(), answered: HashSet::new(), errors: HashMap::new() }
    }

    fn try_one(&mut self, q: &Question, done: &Analysed) -> Option<String> {
        match self.lane.answer(q, done, self.ctx) {
            Ok(v) => {
                self.answers.set(&q.id, v.clone());
                self.answered.insert(q.id.clone());
                self.errors.remove(&q.id);
                Some(v)
            }
            Err(e) => {
                self.errors.insert(q.id.clone(), e);
                None
            }
        }
    }

    /// Answers the open questions routed to delivery `i`; returns the ones newly answered.
    pub fn on_delivery(&mut self, i: usize, done: &Analysed) -> Answers {
        let mut new = Answers::new();
        if !done.is_ready(i) {
            return new;
        }
        for q in self.questions {
            if !self.answered.contains(&q.id) && self.lane.delivery_of(q, done) == Some(i) {
                if let Some(v) = self.try_one(q, done) {
                    new.set(&q.id, v);
                }
            }
        }
        new
    }

    /// Answers, or retries, every question still open, with all analyses present.
    pub fn finish(&mut self, done: &Analysed) {
        for q in self.questions {
            if !self.answered.contains(&q.id) {
                self.try_one(q, done);
            }
        }
    }

    /// Open failures, in question order.
    pub fn errors(&self) -> Vec<(String, String)> {
        self.questions.iter().filter_map(|q| self.errors.get(&q.id).map(|e| (q.id.clone(), e.clone()))).collect()
    }
}

/// The ids of `sent` that the server confirmed as banked.
fn banked(sent: &Answers, r: &SubmitResult) -> Vec<String> {
    if r.correct() {
        return sent.iter().map(|(k, _)| k.to_string()).collect();
    }
    if r.result != "accepted" {
        return Vec::new();
    }
    sent.iter().map(|(k, _)| k).filter(|k| r.accepted_ids.iter().any(|a| a == k)).map(str::to_string).collect()
}

fn preflight(opts: &Opts, pool: &Arc<Pool>, route: Option<&Route>) {
    match route {
        None => say!("preflight", "no cached route for {} (`ctf-run routes --refresh`)", opts.challenge),
        Some(r) => say!(
            "preflight",
            "route: run_for_time_supported={} deadline={:?}ms ranked_pool={:?} regular_pool={:?} sets={:?}/{:?} solved={}{}",
            r.run_for_time_supported,
            r.ranked_deadline_ms,
            r.ranked_pool_size,
            r.regular_pool_size,
            r.ranked_set_count,
            r.regular_set_count,
            r.solved,
            if !r.run_for_time_supported && Mode::split(&opts.challenge).1 != Mode::Single { " (timed by default)" } else { "" }
        ),
    }
    // The status call doubles as a warm-up and shows any live attempt. The server's tail can make
    // it take 20+ s, so it runs detached and is only waited on briefly.
    let (tx, rx) = mpsc::channel();
    let pool2 = pool.clone();
    let challenge = opts.challenge.clone();
    std::thread::spawn(move || {
        let _ = tx.send(Gm { pool: &pool2 }.status(&challenge).map_err(|e| e.to_string()));
    });
    match rx.recv_timeout(STATUS_WAIT) {
        Ok(Ok((v, resp))) => say!(
            "preflight",
            "status HTTP {} ttfb {:.0} ms: result={} deadline={} attempt={}",
            resp.status,
            ms(resp.ttfb),
            v.str_of("result").unwrap_or("-"),
            v.str_of("deadline").unwrap_or("-"),
            v.str_of("attempt_id").unwrap_or("-")
        ),
        Ok(Err(e)) => say!("preflight", "status failed: {e}"),
        Err(_) => say!("preflight", "status still pending after {:?}; not waiting", STATUS_WAIT),
    }
}

/// Fires `n` concurrent `status` requests (each on its own pooled connection) and waits for them,
/// bounded, so the server side (serverless instances, the proxy's upstream connections) is warm
/// when `start` and the timed requests arrive. Stragglers are abandoned with their connections.
fn warm_burst(pool: &Arc<Pool>, challenge: &str, n: usize) {
    let t = Instant::now();
    let (tx, rx) = mpsc::channel();
    for _ in 0..n {
        let (tx, pool, challenge) = (tx.clone(), pool.clone(), challenge.to_string());
        std::thread::spawn(move || {
            let r = pool.get_query(&format!("op=status&challenge={}", gm::query_escape(&challenge)));
            let _ = tx.send(r.map(|r| ms(r.ttfb)).ok());
        });
    }
    drop(tx);
    let mut ttfbs = Vec::new();
    while let Ok(r) = rx.recv_timeout(Duration::from_millis(3000).saturating_sub(t.elapsed())) {
        ttfbs.extend(r);
        if ttfbs.len() == n {
            break;
        }
    }
    ttfbs.sort_by(f64::total_cmp);
    say!("warm", "{}/{} warm-up requests in {:.0} ms (ttfb {:?})", ttfbs.len(), n, ms(t.elapsed()), ttfbs.iter().map(|x| x.round() as i64).collect::<Vec<_>>());
}

pub fn attempt(opts: &Opts, pool: &Arc<Pool>) -> Res<bool> {
    let mut ctx = RunCtx::new(&opts.challenge);
    let lane = lanes::find(opts.lane.as_deref(), &ctx.base);
    match (lane, opts.dry) {
        (Some(l), _) => say!("lane", "{}", l.name()),
        (None, true) => say!("lane", "none for {:?}: dry run downloads only", ctx.base),
        (None, false) => return Err(format!("no lane for {:?} (use --lane, or --dry to only download)", ctx.base).into()),
    }
    let route = gm::cached_routes().and_then(|rs| rs.into_iter().find(|r| r.id == opts.challenge));
    check_route(ctx.mode, opts.op, opts.timed, opts.dry, opts.one_shot_ok, route.as_ref())?;
    if std::env::var_os("CTF_NO_RANKED").is_some() {
        check_unranked(ctx.mode, opts.op, opts.timed, opts.dry, opts.one_shot_ok, route.as_ref())?;
    }

    let sets = route.as_ref().and_then(|r| if opts.timed { r.ranked_set_count } else { r.regular_set_count.or(r.ranked_set_count) });
    let warm = opts.jobs.unwrap_or_else(|| sets.unwrap_or(4).clamp(4, MAX_JOBS as i64) as usize) * if opts.hedge { 2 } else { 1 };
    pool.warm(warm);
    if opts.preflight {
        preflight(opts, pool, route.as_ref());
    }
    pool.warm(warm);
    say!("pool", "{} warm connection(s)", pool.idle_count());
    if opts.warm > 0 {
        warm_burst(pool, &opts.challenge, opts.warm);
    }

    // ---- critical path starts here ----
    let gm = Gm { pool: pool.as_ref() };
    let mut st = Stamps { start_sent: time::now_ms(), ..Stamps::default() };
    let t_start = Instant::now();
    let (mut attempt, start_resp) = gm.start(&opts.challenge, opts.op, opts.timed)?;
    st.start_recv = time::now_ms();
    st.start_date = start_resp.header("date").and_then(time::parse_http_date);
    let start_ms = ms(t_start.elapsed());
    if attempt.deliveries.is_empty() && attempt.raw.str_of("result") == Some("CORRECT") {
        // Reconnect to a solved attempt: re-fetch its material via status (testing / re-solving).
        let (v, resp) = gm.status(&opts.challenge)?;
        attempt.adopt_status(&v)?;
        say!("start", "attempt already solved; deliveries taken from status (ttfb {:.0} ms)", ms(resp.ttfb));
    }
    say!(
        "start",
        "{} HTTP {} ttfb {:.0} ms{}, {} B ({} on wire): {} question(s), {} deliver{} [{}], deadline {}",
        opts.op.name(),
        start_resp.status,
        ms(start_resp.ttfb),
        if start_resp.reused { " (warm connection)" } else { " (new connection)" },
        start_resp.body.len(),
        start_resp.wire_len,
        attempt.questions.len(),
        attempt.deliveries.len(),
        if attempt.deliveries.len() == 1 { "y" } else { "ies" },
        attempt.shape,
        attempt.deadline.as_deref().unwrap_or("none")
    );

    ctx.started_ms = attempt.started_at.as_deref().and_then(time::parse_iso);
    let queries: Vec<String> = attempt.deliveries.iter().map(gm::download_query).collect();
    let jobs = opts.jobs.unwrap_or(queries.len()).clamp(1, MAX_JOBS);
    let next = AtomicUsize::new(0);
    let specs = &attempt.deliveries;
    let work = |i: usize, r: io::Result<Response>| -> Landed {
        let fetch = match r {
            Ok(resp) if resp.ok() => {
                let t = FetchTiming { ttfb_ms: ms(resp.ttfb), total_ms: ms(resp.total), wire: resp.wire_len, done_at_ms: ms(t_start.elapsed()) };
                Ok((delivery_from(&specs[i], resp), t))
            }
            Ok(resp) => Err(format!("HTTP {}: {}", resp.status, resp.text().chars().take(200).collect::<String>())),
            Err(e) => Err(e.to_string()),
        };
        let t = Instant::now();
        let analysis = match (&fetch, lane) {
            (Ok((d, _)), Some(l)) => Some(l.analyse(d, &ctx)),
            _ => None,
        };
        Landed { index: i, fetch, analysis, analyse_ms: ms(t.elapsed()) }
    };

    let mut done = Analysed::new(queries.len());
    let mut answerer = lane.map(|l| Answerer::new(l, &ctx, &attempt.questions));
    let mut confirmed: HashSet<String> = attempt.accepted_ids.iter().cloned().collect();
    let mut submits: Vec<(String, SubmitResult, f64)> = Vec::new();
    let mut fetch_failures = Vec::new();
    let mut timings: Vec<Value> = Vec::new();
    let early = opts.early_submit && !opts.dry && lane.is_some_and(|l| l.self_validating());

    std::thread::scope(|scope| {
        let (tx, rx) = mpsc::channel();
        fetch::spawn_all(scope, pool, opts.hedge, &queries, &next, jobs, &work, tx);
        for landed in rx {
            let i = landed.index;
            st.last_delivery = time::now_ms();
            match landed.fetch {
                Err(e) => {
                    say!("download", "#{} FAILED: {e}", i + 1);
                    fetch_failures.push((i, e));
                }
                Ok((d, t)) => {
                    say!(
                        "download",
                        "#{} {} {} B ({} on wire) ttfb {:.0} ms total {:.0} ms; analysed in {:.1} ms",
                        i + 1,
                        d.name,
                        d.bytes.len(),
                        t.wire,
                        t.ttfb_ms,
                        t.total_ms,
                        landed.analyse_ms
                    );
                    timings.push(obj([
                        ("delivery", (i + 1).into()),
                        ("name", d.name.as_str().into()),
                        ("bytes", d.bytes.len().into()),
                        ("wire", t.wire.into()),
                        ("ttfb_ms", t.ttfb_ms.into()),
                        ("total_ms", t.total_ms.into()),
                        ("done_at_ms", t.done_at_ms.into()),
                        ("analyse_ms", landed.analyse_ms.into()),
                    ]));
                    if let Some(Err(e)) = &landed.analysis {
                        say!("analyse", "#{} {}: {e}", i + 1, d.name);
                    }
                    let analysis = landed.analysis.unwrap_or_else(|| Err("no lane".into()));
                    done.items[i] = Some(Done { delivery: d, analysis });
                }
            }
            // Answer what this delivery makes answerable (failures are retried at the end).
            let Some(ans) = answerer.as_mut() else { continue };
            let a = ans.on_delivery(i, &done);
            let all_in = done.items.iter().all(Option::is_some);
            if early && !all_in && !a.is_empty() {
                let t = Instant::now();
                match gm.submit(&opts.challenge, &a) {
                    Ok((r, resp)) => {
                        let ok = banked(&a, &r);
                        say!(
                            "submit",
                            "early #{}: {} - {}/{} banked, remaining={:?}, ttfb {:.0} ms",
                            i + 1,
                            r.result,
                            ok.len(),
                            a.len(),
                            r.remaining,
                            ms(resp.ttfb)
                        );
                        confirmed.extend(ok);
                        submits.push((format!("early-{}", i + 1), r, ms(t.elapsed())));
                    }
                    Err(e) => say!("submit", "early #{} failed: {e}", i + 1),
                }
            }
        }
    });

    let (answers, errors) = match answerer.as_mut() {
        Some(ans) => {
            ans.finish(&done);
            (ans.answers.clone(), ans.errors())
        }
        None => (Answers::new(), Vec::new()),
    };
    let missing: Vec<&String> = attempt.answer_ids.iter().filter(|id| answers.get(id).is_none()).collect();
    say!(
        "solve",
        "{} answer(s), {} error(s), {} missing, {} fetch failure(s) at {:.0} ms after start",
        answers.len(),
        errors.len(),
        missing.len(),
        fetch_failures.len(),
        ms(t_start.elapsed())
    );
    for (id, e) in errors.iter().take(40) {
        say!("error", "{id}: {e}");
    }
    if errors.len() > 40 {
        say!("error", "... {} more (see errors.json)", errors.len() - 40);
    }

    // Resubmitting an already-accepted answer is harmless, so send everything not confirmed.
    let pending = answers.filter(|id| !confirmed.contains(id));
    let mut ok = false;
    if opts.dry {
        say!("submit", "skipped (--dry)");
        println!("{}", pending.to_value().pretty());
    } else if lane.is_none() {
        say!("submit", "skipped (no lane)");
    } else if (!errors.is_empty() || !missing.is_empty()) && !opts.partial {
        say!("submit", "REFUSED: {} unanswered question(s); --partial submits the rest", errors.len().max(missing.len()));
    } else if pending.is_empty() {
        say!("submit", "nothing left to submit");
        ok = submits.last().is_some_and(|(_, r, _)| r.correct());
    } else {
        let t = Instant::now();
        st.submit_sent = time::now_ms();
        let (r, resp) = gm.submit(&opts.challenge, &pending)?;
        st.submit_recv = time::now_ms();
        st.submit_date = resp.header("date").and_then(time::parse_http_date);
        say!("submit", "{} ({} answers) HTTP {} ttfb {:.0} ms; start->result {:.0} ms", r.result, pending.len(), resp.status, ms(resp.ttfb), ms(t_start.elapsed()));
        println!("{}", gm::trim(&r.raw));
        ok = r.correct();
        submits.push(("final".into(), r, ms(t.elapsed())));
    }
    // ---- critical path ends here ----

    report_clock(&attempt, &st);
    match persist(opts, &attempt, &start_resp, &done, &answers, &errors, &submits, &timings, start_ms, &st) {
        Ok(d) => say!("saved", "{}", d.display()),
        Err(e) => say!("saved", "FAILED: {e}"),
    }
    Ok(ok)
}

/// Where the server's timer sits relative to our requests (positive = after).
fn report_clock(a: &Attempt, st: &Stamps) {
    let Some(started) = a.started_at.as_deref().and_then(time::parse_iso) else { return };
    let rel = |t: i64| format!("{:+.0} ms", (t - started) as f64);
    say!(
        "clock",
        "vs server started_at: start sent {}, start received {}, last delivery {}, submit sent {}, submit received {}",
        rel(st.start_sent),
        rel(st.start_recv),
        rel(st.last_delivery),
        if st.submit_sent > 0 { rel(st.submit_sent) } else { "-".into() },
        if st.submit_recv > 0 { rel(st.submit_recv) } else { "-".into() }
    );
    if let Some(deadline) = a.deadline.as_deref().and_then(time::parse_iso) {
        say!("clock", "window {:.1} s; submit received with {:.1} s to spare", (deadline - started) as f64 / 1e3, if st.submit_recv > 0 { (deadline - st.submit_recv) as f64 / 1e3 } else { f64::NAN });
    }
}

/// Saves the run as a replayable fixture: `<out>/<base>/<mode>-<attempt8>-<unix>/`.
#[allow(clippy::too_many_arguments)]
fn persist(
    opts: &Opts,
    attempt: &Attempt,
    start_resp: &Response,
    done: &Analysed,
    answers: &Answers,
    errors: &[(String, String)],
    submits: &[(String, SubmitResult, f64)],
    timings: &[Value],
    start_ms: f64,
    st: &Stamps,
) -> io::Result<PathBuf> {
    let (base, mode) = Mode::split(&opts.challenge);
    let id8: String = attempt.attempt_id.as_deref().unwrap_or("noattempt").chars().take(8).collect();
    let dir = opts.out.join(base).join(format!("{}-{id8}-{}", mode.name(), st.start_sent / 1000));
    std::fs::create_dir_all(dir.join("deliveries"))?;
    std::fs::write(dir.join("start.json"), &start_resp.body)?;
    for (i, d) in done.items.iter().enumerate() {
        if let Some(d) = d {
            std::fs::write(dir.join("deliveries").join(format!("{:02}-{}", i + 1, d.delivery.name)), &d.delivery.bytes)?;
        }
    }
    let opt = |s: &Option<String>| s.clone().map(Value::Str).unwrap_or(Value::Null);
    let stamp = |t: i64| if t > 0 { Value::Str(time::iso(t)) } else { Value::Null };
    let meta = obj([
        ("challenge", opts.challenge.as_str().into()),
        ("op", opts.op.name().into()),
        ("run_for_time", opts.timed.into()),
        ("attempt_id", opt(&attempt.attempt_id)),
        ("started_at", opt(&attempt.started_at)),
        ("deadline", opt(&attempt.deadline)),
        ("shape", attempt.shape.as_str().into()),
        ("start_ms", start_ms.into()),
        ("start_ttfb_ms", ms(start_resp.ttfb).into()),
        (
            "clock",
            obj([
                ("start_sent", stamp(st.start_sent)),
                ("start_received", stamp(st.start_recv)),
                ("start_date_header", st.start_date.map(stamp).unwrap_or(Value::Null)),
                ("last_delivery", stamp(st.last_delivery)),
                ("submit_sent", stamp(st.submit_sent)),
                ("submit_received", stamp(st.submit_recv)),
                ("submit_date_header", st.submit_date.map(stamp).unwrap_or(Value::Null)),
            ]),
        ),
        ("downloads", Value::Arr(timings.to_vec())),
    ]);
    std::fs::write(dir.join("meta.json"), meta.pretty())?;
    std::fs::write(dir.join("answers.json"), obj([("answers", answers.to_value())]).pretty())?;
    if !errors.is_empty() {
        let e = Value::Obj(errors.iter().map(|(k, v)| (k.clone(), Value::Str(v.clone()))).collect());
        std::fs::write(dir.join("errors.json"), e.pretty())?;
    }
    if !submits.is_empty() {
        let s = Value::Arr(
            submits.iter().map(|(label, r, took)| obj([("label", label.as_str().into()), ("ms", (*took).into()), ("response", r.raw.clone())])).collect(),
        );
        std::fs::write(dir.join("result.json"), s.pretty())?;
    }
    Ok(dir)
}

/// Lists a saved run's delivery files in order.
pub fn saved_deliveries(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir.join("deliveries"))?.filter_map(|e| Some(e.ok()?.path())).collect();
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(rft: bool, ranked: i64) -> Route {
        Route {
            id: "x/mastery".into(),
            run_for_time_supported: rft,
            ranked_deadline_ms: Some(30000),
            ranked_pool_size: Some(ranked),
            regular_pool_size: Some(if rft { 32 } else { 0 }),
            ranked_set_count: Some(2),
            regular_set_count: Some(2),
            solved: false,
        }
    }

    #[test]
    fn route_guard() {
        use Mode::*;
        use StartOp::*;
        let one_shot = route(false, 1);
        let timed_default = route(false, 2);
        let dual = route(true, 32);
        // Single solve is always fine.
        assert!(check_route(Single, Start, false, true, false, None).is_ok());
        // Unknown routes fail closed outside single-solve.
        assert!(check_route(Mastery, Start, false, true, false, None).is_err());
        assert!(check_route(Mastery, Start, false, false, true, None).is_ok());
        // --dry never starts a clock.
        assert!(check_route(Mastery, Start, false, true, true, Some(&one_shot)).is_err());
        assert!(check_route(TimeTrial, Retry, false, true, false, Some(&timed_default)).is_err());
        // A solved timed-by-default route: a plain start only reconnects, so --dry is a free test; retry is not.
        let solved = Route { solved: true, ..route(false, 2) };
        assert!(check_route(TimeTrial, Start, false, true, false, Some(&solved)).is_ok());
        assert!(check_route(TimeTrial, Retry, false, true, false, Some(&solved)).is_err());
        assert!(check_route(TimeTrial, Start, false, true, false, Some(&timed_default)).is_err());
        // CTF_NO_RANKED: only untimed play gets through.
        assert!(check_unranked(Mastery, Retry, false, false, false, Some(&dual)).is_ok());
        assert!(check_unranked(Mastery, Retry, true, false, false, Some(&dual)).is_err());
        assert!(check_unranked(Mastery, Start, false, false, false, Some(&timed_default)).is_err());
        assert!(check_unranked(Mastery, Start, false, false, true, Some(&one_shot)).is_err());
        assert!(check_unranked(Mastery, Start, false, false, false, None).is_err());
        assert!(check_unranked(Single, Start, false, false, false, None).is_ok());
        assert!(check_unranked(TimeTrial, Start, false, true, false, Some(&solved)).is_ok());
        assert!(check_route(Mastery, Start, true, true, false, Some(&dual)).is_err());
        assert!(check_route(Mastery, Start, false, true, false, Some(&dual)).is_ok());
        assert!(check_route(Mastery, Practice, false, true, false, Some(&one_shot)).is_ok());
        // One-shot routes need the explicit flag.
        assert!(check_route(Mastery, Start, false, false, false, Some(&one_shot)).is_err());
        assert!(check_route(Mastery, Start, false, false, true, Some(&one_shot)).is_ok());
        assert!(check_route(Mastery, Start, false, false, false, Some(&timed_default)).is_ok());
        // run_for_time only where supported.
        assert!(check_route(Mastery, Start, true, false, false, Some(&one_shot)).is_err());
        assert!(check_route(Mastery, Retry, true, false, false, Some(&dual)).is_ok());
    }

    /// Live-path regression: a warmups mastery as per-question files where set 2's unpinned Caesar row (15) sits
    /// in the slug (ambiguous alone) and lands *before* set 1, which pins the row's transform. The
    /// early answer fails; the final pass must retry it with every analysis present.
    #[test]
    fn early_failures_are_retried_once_everything_landed() {
        use crate::codec::{atbash, rot};
        use crate::lanes::warmups::Warmups;
        use crate::model::DeliverySpec;
        let set1 = format!(
            "01\t{}\n02\t{}\n16\t{}\n13\t{}\n",
            "TmljZSB3b3Jr",
            rot(" decoding this warmup! To solve,", 13),
            rot(" submit this exact answer", 7),
            atbash(" to the Game Master: zesty-sunset-quartz-65")
        );
        let set2 = format!(
            "01\t{}\n16\t{}\n",
            "TmljZSB3b3JrIGRlY29kaW5nIHRoaXMgd2FybXVwISBUbyBzb2x2ZSwgc3VibWl0IHRoaXMgZXhhY3QgYW5zd2VyIHRvIHRoZSBHYW1lIE1hc3RlcjogaGFwcHkt",
            rot("piano-whale-26", 7)
        );
        let qs: Vec<Question> = [("set1.message", "blob-01.txt"), ("set2.message", "blob-02.txt")]
            .iter()
            .map(|(id, art)| Question::from_json(&Value::parse(&format!(r#"{{"id":"{id}","artifact":"{art}"}}"#)).unwrap()).unwrap())
            .collect();
        let ctx = RunCtx::new("warmups-day-14/mastery");
        let lane = &Warmups;
        let mut done = Analysed::new(2);
        let mut ans = Answerer::new(lane, &ctx, &qs);
        for (i, name, text) in [(1usize, "blob-02.txt", &set2), (0, "blob-01.txt", &set1)] {
            let spec = DeliverySpec { index: i, link: String::new(), cap: String::new(), filename: None, sha256: None, kind: None, size: None };
            let d = Delivery { spec, name: name.into(), bytes: text.as_bytes().to_vec() };
            let analysis = lane.analyse(&d, &ctx);
            done.items[i] = Some(Done { delivery: d, analysis });
            ans.on_delivery(i, &done);
        }
        assert_eq!(ans.errors().len(), 1, "set 2 cannot be answered on its own");
        ans.finish(&done);
        assert!(ans.errors().is_empty(), "{:?}", ans.errors());
        assert_eq!(ans.answers.get("set2.message"), Some("happy-piano-whale-26"));
        assert_eq!(ans.answers.get("set1.message"), Some("zesty-sunset-quartz-65"));
    }

    #[test]
    fn banking_uses_accepted_ids() {
        let mut sent = Answers::new();
        sent.set("set1.a", "x");
        sent.set("set1.b", "y");
        let r = SubmitResult::from_value(Value::parse(r#"{"result":"accepted","remaining":3,"accepted_ids":["set0.z","set1.a"]}"#).unwrap());
        assert_eq!(banked(&sent, &r), ["set1.a"]);
        let r = SubmitResult::from_value(Value::parse(r#"{"result":"incorrect"}"#).unwrap());
        assert!(banked(&sent, &r).is_empty());
        let r = SubmitResult::from_value(Value::parse(r#"{"result":"CORRECT"}"#).unwrap());
        assert_eq!(banked(&sent, &r).len(), 2);
    }
}
