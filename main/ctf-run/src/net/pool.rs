//! A pool of keep-alive connections. Connections are opened (and optionally warmed) before the
//! timed `start`, so nothing on the critical path pays for a connect or TLS handshake.

use super::http::{Client, Endpoint, Response};
use std::io;
use std::sync::{Arc, Mutex};

pub struct Pool {
    pub ep: Arc<Endpoint>,
    idle: Mutex<Vec<Client>>,
}

impl Pool {
    pub fn new(ep: Arc<Endpoint>) -> Arc<Pool> {
        Arc::new(Pool { ep, idle: Mutex::new(Vec::new()) })
    }

    /// Opens up to `n` connections in parallel so that `n` requests can start at once.
    /// Failures are ignored: a missing connection is opened on demand later.
    pub fn warm(&self, n: usize) {
        let have = self.idle.lock().unwrap().len();
        let want = n.saturating_sub(have);
        let opened: Vec<Client> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..want).map(|_| s.spawn(|| Client::connected(&self.ep))).collect();
            hs.into_iter().filter_map(|h| h.join().ok()?.ok()).collect()
        });
        self.idle.lock().unwrap().extend(opened);
    }

    /// Checks out a connection (an open one if available).
    pub fn get(&self) -> Client {
        let mut idle = self.idle.lock().unwrap();
        // Prefer open connections; the vector holds only clients that were open when returned.
        idle.pop().unwrap_or_else(|| Client::idle(&self.ep))
    }

    /// Returns a connection; closed ones are dropped.
    pub fn put(&self, c: Client) {
        if c.is_open() {
            self.idle.lock().unwrap().push(c);
        }
    }

    pub fn idle_count(&self) -> usize {
        self.idle.lock().unwrap().len()
    }

    pub fn get_query(&self, query: &str) -> io::Result<Response> {
        let mut c = self.get();
        let r = c.get(query);
        self.put(c);
        r
    }

    pub fn post_json(&self, body: &str) -> io::Result<Response> {
        let mut c = self.get();
        let r = c.post_json(body);
        self.put(c);
        r
    }
}
