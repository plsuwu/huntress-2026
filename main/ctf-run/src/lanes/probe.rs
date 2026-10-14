//! `probe`: answers nothing. Exercises the network pipeline (`--lane probe --dry`) and reports what
//! each delivery looks like. Never registered for a challenge prefix.

use super::{Analysed, Analysis, Lane};
use crate::model::{Delivery, Question, RunCtx};

pub struct Probe;

pub struct Seen {
    pub len: usize,
    pub magic: String,
}

impl Lane for Probe {
    fn name(&self) -> &'static str {
        "probe"
    }

    fn prefixes(&self) -> &'static [&'static str] {
        &[]
    }

    fn analyse(&self, d: &Delivery, _: &RunCtx) -> Result<Analysis, String> {
        let magic = d.bytes.iter().take(8).map(|b| format!("{b:02x}")).collect();
        Ok(Box::new(Seen { len: d.bytes.len(), magic }))
    }

    fn answer(&self, q: &Question, done: &Analysed, _: &RunCtx) -> Result<String, String> {
        let seen: Vec<String> = done.all::<Seen>().map(|(i, s)| format!("#{} {}B {}", i + 1, s.len, s.magic)).collect();
        Err(format!("probe lane answers nothing ({}; deliveries: {})", q.key, seen.join(", ")))
    }
}
