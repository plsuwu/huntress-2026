// Reference excerpt (from a working Rust solver): the guards every `start` goes through. They fail closed: anything
// that could spend a ranked/one-shot attempt or start a clock without submitting needs an explicit flag, and the
// NO_RANKED environment variable (set for all automation and lane agents) refuses ranked starts outright.
//   NO_RANKED=hof -> check_no_hof: only `--timed` (a leaderboard attempt on a dual-pool route) is refused;
//   NO_RANKED=<anything else> -> check_unranked: everything that could spend a ranked attempt is refused.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode { Single, TimeTrial, Mastery }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StartOp { Start, Retry, Practice }

/// One row of the route cache (from the API's challenge list).
#[derive(Clone, Debug)]
pub struct Route {
    pub id: String,
    pub run_for_time_supported: bool, // dual-pool: untimed regular attempts + opt-in ranked ones
    pub ranked_deadline_ms: Option<i64>,
    pub ranked_pool_size: Option<i64>,
    pub regular_pool_size: Option<i64>,
    pub ranked_set_count: Option<i64>,
    pub regular_set_count: Option<i64>,
    pub solved: bool,
}

impl Route {
    /// Timed by default with a ranked pool of one: the route's only attempt.
    pub fn one_shot(&self) -> bool {
        !self.run_for_time_supported && self.ranked_pool_size == Some(1)
    }
}

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
            Err("route not in the cache (`<solver> routes --refresh`); pass --one-shot-ok to start it blind".into())
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

/// `NO_RANKED=hof` (release automation and lane agents): solves come first, so one-shot and
/// timed-by-default routes may be started; only `--timed` - a leaderboard attempt on a dual-pool route, which the
/// coordinator's campaign owns - is refused.
pub fn check_no_hof(timed: bool) -> Result<(), String> {
    if timed {
        return Err("NO_RANKED=hof: --timed starts a leaderboard attempt (the coordinator's campaign)".into());
    }
    Ok(())
}

/// `NO_RANKED=1` (any value but `hof`): refuse anything that could spend a ranked attempt -
/// `--timed`, `--one-shot-ok`, and non-dry starts/retries on timed-by-default or uncached routes. Untimed
/// regular play (dual-pool routes), single-solve, practice and dry reconnects stay allowed.
pub fn check_unranked(mode: Mode, op: StartOp, timed: bool, dry: bool, one_shot_ok: bool, route: Option<&Route>) -> Result<(), String> {
    let deny = |why: &str| Err(format!("NO_RANKED is set: {why} (ranked attempts are the coordinator's call)"));
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
        // NO_RANKED: only untimed play gets through.
        assert!(check_unranked(Mastery, Retry, false, false, false, Some(&dual)).is_ok());
        assert!(check_unranked(Mastery, Retry, true, false, false, Some(&dual)).is_err());
        assert!(check_unranked(Mastery, Start, false, false, false, Some(&timed_default)).is_err());
        assert!(check_unranked(Mastery, Start, false, false, true, Some(&one_shot)).is_err());
        assert!(check_unranked(Mastery, Start, false, false, false, None).is_err());
        assert!(check_unranked(Single, Start, false, false, false, None).is_ok());
        assert!(check_unranked(TimeTrial, Start, false, true, false, Some(&solved)).is_ok());
        // NO_RANKED=hof: only --timed (leaderboard) is refused.
        assert!(check_no_hof(true).is_err());
        assert!(check_no_hof(false).is_ok());
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

}
