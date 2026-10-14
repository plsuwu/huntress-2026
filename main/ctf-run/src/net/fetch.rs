//! Parallel downloads (identity encoding) over pooled connections. Each worker runs a caller-supplied step on its own
//! result (e.g. parse + analyse the delivery) before sending it on, so CPU work overlaps the
//! downloads still in flight and the receiver sees finished items in completion order.

use super::http::Response;
use super::pool::Pool;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::thread::Scope;

/// Spawns `jobs` workers inside `scope`; each takes the next query (`op=...` string), fetches it,
/// runs `then(index, result)` and sends the output on `tx`. Returns immediately; the receiver
/// gets exactly `queries.len()` messages unless a worker panics. `next` is the shared work cursor
/// (start it at 0).
pub fn spawn_all<'scope, 'env: 'scope, T, F>(
    scope: &'scope Scope<'scope, 'env>,
    pool: &'env Pool,
    queries: &'env [String],
    next: &'env AtomicUsize,
    jobs: usize,
    then: &'env F,
    tx: Sender<T>,
) where
    T: Send + 'env,
    F: Fn(usize, io::Result<Response>) -> T + Sync,
{
    for _ in 0..jobs.clamp(1, queries.len().max(1)) {
        let tx = tx.clone();
        scope.spawn(move || {
            let mut client = pool.get();
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(q) = queries.get(i) else { break };
                let result = client.get_identity(q);
                if tx.send(then(i, result)).is_err() {
                    break;
                }
            }
            pool.put(client);
        });
    }
}
