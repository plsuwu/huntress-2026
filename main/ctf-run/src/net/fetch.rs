//! Parallel downloads (identity encoding) over pooled connections. Each worker runs a caller-supplied step on its own
//! result (e.g. parse + analyse the delivery) before sending it on, so CPU work overlaps the
//! downloads still in flight and the receiver sees finished items in completion order.

use super::http::{Progress, Response};
use super::pool::Pool;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::Scope;

/// Spawns `jobs` workers inside `scope`; each takes the next query (`op=...` string), opens
/// `open(index)` (an optional sink that sees the body while it downloads; not with `hedge`), fetches it,
/// runs `then(index, result, sink)` and sends the output on `tx`. Returns immediately; the receiver
/// gets exactly `queries.len()` messages unless a worker panics. `next` is the shared work cursor
/// (start it at 0).
#[allow(clippy::too_many_arguments)]
pub fn spawn_all<'scope, 'env: 'scope, T, S, O, F>(
    scope: &'scope Scope<'scope, 'env>,
    pool: &'env Arc<Pool>,
    hedge: bool,
    queries: &'env [String],
    next: &'env AtomicUsize,
    jobs: usize,
    open: &'env O,
    then: &'env F,
    tx: Sender<T>,
) where
    T: Send + 'env,
    S: Progress,
    O: Fn(usize) -> Option<S> + Sync,
    F: Fn(usize, io::Result<Response>, Option<S>) -> T + Sync,
{
    for _ in 0..jobs.clamp(1, queries.len().max(1)) {
        let tx = tx.clone();
        scope.spawn(move || {
            let mut client = pool.get();
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(q) = queries.get(i) else { break };
                let mut sink = if hedge { None } else { open(i) };
                let result = match sink.as_mut() {
                    _ if hedge => get_hedged(pool, q),
                    Some(s) => client.get_identity_progress(q, Some(s as &mut dyn Progress)),
                    None => client.get_identity(q),
                };
                if tx.send(then(i, result, sink)).is_err() {
                    break;
                }
            }
            pool.put(client);
        });
    }
}

/// The same GET on two pooled connections; the first successful response wins (a failure waits for
/// the other copy). Both copies run on detached threads, so the loser never holds up the caller -
/// a scoped thread would be joined before the submit. Measured on d15 mobile mastery (5 x 12 KB,
/// jumpbox): the last delivery landed 16-71 ms sooner (mean ~37 ms); the server's straggler tail
/// (a 465 ms download in one ranked run) is what this is for. Small deliveries only: it doubles bandwidth.
pub fn get_hedged(pool: &Arc<Pool>, q: &str) -> io::Result<Response> {
    let (tx, rx) = mpsc::channel();
    for _ in 0..2 {
        let (pool, q, tx) = (Arc::clone(pool), q.to_string(), tx.clone());
        std::thread::spawn(move || {
            let mut c = pool.get();
            let r = c.get_identity(&q);
            pool.put(c);
            let _ = tx.send(r);
        });
    }
    drop(tx);
    let first = rx.recv().map_err(|_| io::Error::other("hedged fetch: no response"))?;
    if matches!(&first, Ok(r) if r.ok()) {
        return first;
    }
    match rx.recv() {
        Ok(Ok(r)) if r.ok() => Ok(r),
        _ => first,
    }
}
