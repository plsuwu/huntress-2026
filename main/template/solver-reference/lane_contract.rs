// Reference excerpt (from a working Rust solver): the lane contract. `model::{Delivery, Question, RunCtx}` are the
// solver's own types: a downloaded artifact (name + bytes), a question from the start response (id, text, artifact
// label, answer_format), and per-attempt context (e.g. the attempt's start time, for gating grader eras).

//! Lanes: one solver per CTF lane, not per challenge.
//!
//! A lane works in two stages:
//!   1. `analyse` runs once per delivery, as soon as it is downloaded (on a fetch worker thread),
//!      and parses everything the lane will need from it into the lane's own analysis type.
//!   2. `answer` runs once per question against the finished analyses, matching the question-id
//!      suffix against the lane's kind table (a `match` or a `(suffix, handler)` list): adding a
//!      question kind is adding one row.
//!
//! Lanes must never guess: return `Err` (with what was seen) when unsure; the runner then refuses
//! to submit and prints the error so the kind/reader can be added.


use crate::model::{Delivery, Question, RunCtx};
use std::any::Any;

pub type Analysis = Box<dyn Any + Send + Sync>;

pub trait Lane: Sync {
    fn name(&self) -> &'static str;

    /// Challenge-id prefixes this lane owns (`"alpha-day-"`).
    fn prefixes(&self) -> &'static [&'static str];

    /// Parses one delivery. Called concurrently for different deliveries.
    fn analyse(&self, d: &Delivery, ctx: &RunCtx) -> Result<Analysis, String>;

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

// The registry: every lane module, plus a `probe` lane that answers nothing (for pipeline testing).
// pub mod alpha; pub mod beta; pub mod probe;
static LANES: &[&dyn Lane] = &[/* &alpha::Alpha, &beta::Beta, &probe::Probe */];

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
