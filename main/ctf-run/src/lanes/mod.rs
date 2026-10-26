//! Lanes: one solver per CTF lane, not per challenge.
//!
//! A lane works in two stages:
//!   1. `analyse` runs once per delivery, as soon as it is downloaded (on a fetch worker thread),
//!      and parses everything the lane will need from it into the lane's own analysis type.
//!   2. `answer` runs once per question against the finished analyses, matching the question-id
//!      suffix against the lane's kind table (a `match` or a `(suffix, handler)` list): adding a
//!      question kind is adding one row.
//!
//! A lane may also `stream`: start on a delivery's complete parts while the rest is still downloading
//! (binary-RE: zip members), then `finish` on the whole delivery with the same result `analyse` gives.
//!
//! Lanes must never guess: return `Err` (with what was seen) when unsure; the runner then refuses
//! to submit and prints the error so the kind/reader can be added.

pub mod binary_re;
pub mod metadata;
pub mod mobile;
pub mod probe;
pub mod script_analysis;
pub mod warmups;

use crate::model::{Delivery, Question, RunCtx};
use std::any::Any;

pub type Analysis = Box<dyn Any + Send + Sync>;

/// Incremental analysis of one delivery while it downloads (see `Lane::stream`).
pub trait Stream {
    /// The body received so far (a growing prefix; identity-encoded downloads only).
    fn feed(&mut self, prefix: &[u8]);
    /// The complete delivery: returns exactly what `Lane::analyse` would.
    fn finish(self: Box<Self>, d: &Delivery, ctx: &RunCtx) -> Result<Analysis, String>;
}

impl crate::net::http::Progress for Box<dyn Stream> {
    fn progress(&mut self, body: &[u8]) {
        self.feed(body);
    }
}

/// The lane's stream for one delivery, unless `CTF_NO_STREAM` is set (A/B timing, or a way out).
pub fn open_stream(lane: &dyn Lane, ctx: &RunCtx) -> Option<Box<dyn Stream>> {
    if std::env::var_os("CTF_NO_STREAM").is_some() { None } else { lane.stream(ctx) }
}

/// Analyses one complete delivery the way a live run does: through the lane's stream when it has one
/// (fed in `step`-byte increments, like a download), else `analyse`. For replay and tests.
pub fn analyse_as_streamed(lane: &dyn Lane, d: &Delivery, ctx: &RunCtx, step: usize) -> Result<Analysis, String> {
    // `CTF_REPLAY_MBPS=17`: pace the feed like a download at that rate and report the analysis tail
    // (last byte -> analysis done), the part of a live run's timer the stream is meant to hide.
    let mbps: Option<f64> = std::env::var("CTF_REPLAY_MBPS").ok().and_then(|v| v.parse().ok()).filter(|r| *r > 0.0);
    let t0 = std::time::Instant::now();
    let tail = |t: std::time::Instant, how: &str| {
        if mbps.is_some() {
            eprintln!("  [paced] {} {} B: download {:.0} ms, analysis tail {:.1} ms ({how})", d.name, d.bytes.len(), (t - t0).as_secs_f64() * 1e3, t.elapsed().as_secs_f64() * 1e3);
        }
    };
    let pace = |k: usize| {
        if let Some(r) = mbps {
            let due = std::time::Duration::from_secs_f64(k as f64 / (r * 1e6));
            if let Some(w) = due.checked_sub(t0.elapsed()) {
                std::thread::sleep(w);
            }
        }
    };
    let Some(mut s) = open_stream(lane, ctx) else {
        pace(d.bytes.len());
        let t = std::time::Instant::now();
        let r = lane.analyse(d, ctx);
        tail(t, "analyse");
        return r;
    };
    let mut k = 0;
    while k < d.bytes.len() {
        k = (k + step.max(1)).min(d.bytes.len());
        pace(k);
        s.feed(&d.bytes[..k]);
    }
    let t = std::time::Instant::now();
    let r = s.finish(d, ctx);
    tail(t, "stream");
    r
}

pub trait Lane: Sync {
    fn name(&self) -> &'static str;

    /// Challenge-id prefixes this lane owns (`"warmups-day-"`).
    fn prefixes(&self) -> &'static [&'static str];

    /// Parses one delivery. Called concurrently for different deliveries.
    fn analyse(&self, d: &Delivery, ctx: &RunCtx) -> Result<Analysis, String>;

    /// A stream for one delivery that is about to download, or `None` to `analyse` it once it lands.
    fn stream(&self, _ctx: &RunCtx) -> Option<Box<dyn Stream>> {
        None
    }

    /// Answers one question from the finished analyses.
    fn answer(&self, q: &Question, done: &Analysed, ctx: &RunCtx) -> Result<String, String>;

    /// The delivery a question depends on, so it can be answered (and, with `--early-submit`,
    /// submitted) before the other deliveries land. `None`: answer once everything is in.
    fn delivery_of(&self, q: &Question, done: &Analysed) -> Option<usize> {
        default_delivery_of(q, done.len())
    }

    /// True when every answer the lane returns has been checked against a format it fully
    /// determines (template, CRC, answer pattern) - only then are early partial submits allowed.
    fn self_validating(&self) -> bool {
        false
    }
}

/// `delivery N: ...` labels map to delivery N; with a single delivery everything maps to it.
pub fn default_delivery_of(q: &Question, deliveries: usize) -> Option<usize> {
    if let Some((n, _)) = q.delivery_label() {
        return n.checked_sub(1).filter(|&i| i < deliveries);
    }
    (deliveries == 1).then_some(0)
}

/// A finished delivery and its analysis.
pub struct Done {
    pub delivery: Delivery,
    pub analysis: Result<Analysis, String>,
}

/// The analyses available so far, indexed by delivery.
pub struct Analysed {
    pub items: Vec<Option<Done>>,
}

impl Analysed {
    pub fn new(n: usize) -> Analysed {
        Analysed { items: (0..n).map(|_| None).collect() }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_ready(&self, i: usize) -> bool {
        self.items.get(i).is_some_and(Option::is_some)
    }

    /// The analysis of delivery `i`, downcast to the lane's type.
    pub fn get<T: 'static>(&self, i: usize) -> Result<&T, String> {
        let done = self.items.get(i).and_then(Option::as_ref).ok_or_else(|| format!("delivery {} not available", i + 1))?;
        match &done.analysis {
            Ok(a) => a.downcast_ref::<T>().ok_or_else(|| "analysis has an unexpected type".to_string()),
            Err(e) => Err(format!("delivery {} ({}) failed analysis: {e}", i + 1, done.delivery.name)),
        }
    }

    /// Every successful analysis of type `T`, with its delivery index.
    pub fn all<T: 'static>(&self) -> impl Iterator<Item = (usize, &T)> {
        self.items.iter().enumerate().filter_map(|(i, d)| {
            let a = d.as_ref()?.analysis.as_ref().ok()?;
            Some((i, a.downcast_ref::<T>()?))
        })
    }
}

static LANES: &[&dyn Lane] = &[&warmups::Warmups, &mobile::Mobile, &metadata::Metadata, &binary_re::BinaryRe, &script_analysis::ScriptAnalysis, &probe::Probe];

pub fn all() -> &'static [&'static dyn Lane] {
    LANES
}

/// The lane named `name`, or the lane owning challenge id `base`.
pub fn find(name: Option<&str>, base: &str) -> Option<&'static dyn Lane> {
    match name {
        Some(n) => LANES.iter().copied().find(|l| l.name() == n),
        None => LANES.iter().copied().find(|l| l.prefixes().iter().any(|p| base.starts_with(p))),
    }
}
