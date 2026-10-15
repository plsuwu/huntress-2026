//! ctf-run: per-lane solver and low-latency runner for the Huntress CTF Game Master.
//!
//! stdout carries results (the submit response, answers on --dry, replay output); progress and
//! timings go to stderr.

#[allow(dead_code)] // shared library surface: fields/helpers future question kinds use
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod codec;
#[allow(dead_code)] // shared library surface: fields/helpers future question kinds use
mod crypto;
#[allow(dead_code)] // shared library surface: fields/helpers future question kinds use
mod evidence;
#[allow(dead_code)] // shared library surface: fields/helpers future question kinds use
mod formats;
#[allow(dead_code)] // shared library surface: fields/helpers future question kinds use
mod gm;
#[allow(dead_code)] // shared library surface: fields/helpers future question kinds use
mod json;
mod lanes;
mod log;
#[allow(dead_code)] // shared library surface: fields/helpers future question kinds use
mod model;
mod net;
mod oracle;
mod replay;
mod run;
#[allow(dead_code)] // shared library surface: fields/helpers future question kinds use
mod rx;
#[allow(dead_code)] // shared library surface: fields/helpers future question kinds use
mod script;
#[allow(dead_code)] // shared library surface: fields/helpers future question kinds use
mod time;

use gm::{Gm, StartOp};
use net::http::Endpoint;
use net::pool::Pool;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage:
  ctf-run [run] <challenge> [options]   start/retry an attempt, download, solve, submit
  ctf-run replay <path>... [--lane L] [--show]
                                        re-solve saved runs / fixture manifests offline and diff
  ctf-run oracle <run-dir> [--chunk N] localise wrong answers by subset submits (untimed attempts only)
  ctf-run status <challenge>            current attempt state
  ctf-run routes [--refresh] [FILTER]   route metadata (cached; --refresh re-reads op=list, 8-24 s)
  ctf-run lanes                         list lanes
  ctf-run bench <zip> [password]        time zip parsing/decryption/inflate of every member

run options:
  -t, --time-trial     append /time_trial          -m, --mastery     append /mastery
  -r, --retry          op=retry (next variant)     --practice        op=practice (legacy continuation)
  --timed              run_for_time:true (dual-pool routes: ranked Hall of Fame attempt)
  -n, --dry            solve but do not submit; print the answers
  --early-submit       submit each delivery's answers as soon as they are ready (self-validating lanes)
  --partial            submit what was answered even if some questions failed
  --lane NAME          force a lane (e.g. probe)
  -j, --jobs N         parallel downloads (default: one per delivery, max 64)
  -o, --out DIR        where runs are saved (default ./runs)
  -u, --url URL        endpoint (default $HUNTRESS_GM_URL, else the CourseStack proxy)
  --one-shot-ok        allow starting a one-shot route, or a route missing from the cache
  --no-preflight       skip the route check and status warm-up
  --warm N             fire N concurrent status requests just before start (warms the server side)

environment:
  HUNTRESS_GM_URL      default endpoint (https://... needs a build with --features tls)
  HUNTRESS_GM_TOKEN    bearer token; only ever sent over https";

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn endpoint(url: Option<String>) -> Res<std::sync::Arc<Endpoint>> {
    let url = url
        .or_else(|| std::env::var("HUNTRESS_GM_URL").ok())
        .unwrap_or_else(|| gm::PROXY_URL.to_string());
    let token = std::env::var("HUNTRESS_GM_TOKEN").ok().filter(|t| !t.is_empty());
    let ep = Endpoint::parse(&url, token.as_deref())?;
    if ep.https && !ep.has_auth() {
        say!("auth", "HUNTRESS_GM_TOKEN is not set: no Authorization header");
    }
    Ok(ep)
}

fn real_main() -> Res<bool> {
    log::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return Ok(true);
    }
    let cmd = match args[0].as_str() {
        "run" | "replay" | "oracle" | "status" | "routes" | "lanes" | "bench" => args.remove(0),
        _ => "run".to_string(),
    };
    let mut url = None;
    let mut it = args.into_iter();
    let mut pos: Vec<String> = Vec::new();
    let mut o = run::Opts {
        challenge: String::new(),
        op: StartOp::Start,
        timed: false,
        dry: false,
        early_submit: false,
        partial: false,
        lane: None,
        jobs: None,
        out: PathBuf::from("runs"),
        one_shot_ok: false,
        preflight: true,
        warm: 0,
    };
    let (mut suffix, mut refresh, mut show, mut chunk) = ("", false, false, 64usize);
    while let Some(a) = it.next() {
        let mut val = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "-t" | "--time-trial" => suffix = "/time_trial",
            "-m" | "--mastery" => suffix = "/mastery",
            "-r" | "--retry" => o.op = StartOp::Retry,
            "--practice" => o.op = StartOp::Practice,
            "--timed" => o.timed = true,
            "-n" | "--dry" => o.dry = true,
            "--early-submit" => o.early_submit = true,
            "--partial" => o.partial = true,
            "--lane" => o.lane = Some(val("--lane")?),
            "-j" | "--jobs" => o.jobs = Some(val("--jobs")?.parse().map_err(|_| "--jobs takes a number")?),
            "-o" | "--out" => o.out = PathBuf::from(val("--out")?),
            "-u" | "--url" => url = Some(val("--url")?),
            "--one-shot-ok" => o.one_shot_ok = true,
            "--no-preflight" => o.preflight = false,
            "--warm" => o.warm = val("--warm")?.parse().map_err(|_| "--warm takes a number")?,
            "--refresh" => refresh = true,
            "--show" => show = true,
            "--chunk" => chunk = val("--chunk")?.parse().map_err(|_| "--chunk takes a number")?,
            s if s.starts_with('-') => return Err(format!("unknown option {s}\n\n{USAGE}").into()),
            _ => pos.push(a),
        }
    }

    match cmd.as_str() {
        "bench" => {
            let path = pos.first().ok_or("bench needs a zip")?;
            let pw = pos.get(1).map(String::as_str).unwrap_or("infected");
            let data = std::fs::read(path)?;
            let t = std::time::Instant::now();
            let z = formats::zip::Zip::parse(&data)?;
            let mut total = 0usize;
            let mut hash_time = std::time::Duration::ZERO;
            for e in z.files() {
                let t1 = std::time::Instant::now();
                let out = z.read(e, pw.as_bytes())?;
                total += out.len();
                let th = std::time::Instant::now();
                std::hint::black_box(crypto::sha256::sha256(&out));
                hash_time += th.elapsed();
                say!("bench", "{} {} -> {} B in {:.1} ms (method {}, aes {:?})", e.name, e.csize, out.len(), log::ms(t1.elapsed()), e.method, e.aes);
            }
            let secs = t.elapsed().as_secs_f64();
            let secs = secs - hash_time.as_secs_f64();
            say!("bench", "{} B in {:.1} ms = {:.0} MB/s (output, decrypt+inflate)", total, secs * 1e3, total as f64 / secs / 1e6);
            let hs = hash_time.as_secs_f64();
            say!("bench", "sha256 {:.1} ms = {:.0} MB/s", hs * 1e3, total as f64 / hs / 1e6);
            // WinZip AES key derivation (PBKDF2-HMAC-SHA1, 1000 rounds, 2 blocks = the AES-256 key).
            let (salt, n) = ([7u8; 16], 200);
            let t = std::time::Instant::now();
            for _ in 0..n {
                std::hint::black_box(crypto::sha1::pbkdf2_hmac_sha1(b"infected", &salt, 1000, 40));
            }
            let ours = t.elapsed().as_secs_f64() * 1e3 / n as f64;
            let t = std::time::Instant::now();
            let mut out = [0u8; 40];
            for _ in 0..n {
                ring::pbkdf2::derive(ring::pbkdf2::PBKDF2_HMAC_SHA1, std::num::NonZeroU32::new(1000).unwrap(), &salt, b"infected", &mut out);
                std::hint::black_box(&out);
            }
            let ringt = t.elapsed().as_secs_f64() * 1e3 / n as f64;
            assert_eq!(out.to_vec(), crypto::sha1::pbkdf2_hmac_sha1(b"infected", &salt, 1000, 40));
            say!("bench", "pbkdf2 (40 B): ours {ours:.3} ms, ring {ringt:.3} ms per key");
            Ok(true)
        }
        "lanes" => {
            for l in lanes::all() {
                println!("{:<12} {}", l.name(), l.prefixes().join(" "));
            }
            Ok(true)
        }
        "replay" => {
            if pos.is_empty() {
                return Err("replay needs at least one path".into());
            }
            let paths: Vec<PathBuf> = pos.iter().map(PathBuf::from).collect();
            Ok(replay::main(&paths, o.lane.as_deref(), show))
        }
        "oracle" => {
            let dir = PathBuf::from(pos.first().ok_or("oracle needs a run directory")?);
            let pool = Pool::new(endpoint(url)?);
            oracle::main(&dir, &pool, chunk)
        }
        "status" => {
            let c = pos.first().ok_or("status needs a challenge")?.clone() + suffix;
            let pool = Pool::new(endpoint(url)?);
            let (v, resp) = Gm { pool: &pool }.status(&c)?;
            say!("status", "HTTP {} ttfb {:.0} ms", resp.status, log::ms(resp.ttfb));
            println!("{}", json::Value::parse(&gm::trim(&v))?.pretty());
            Ok(resp.ok())
        }
        "routes" => {
            let routes = match (refresh, gm::cached_routes()) {
                (false, Some(r)) => r,
                _ => {
                    let pool = Pool::new(endpoint(url)?);
                    let (v, resp) = Gm { pool: &pool }.list()?;
                    say!("routes", "op=list HTTP {} in {:.0} ms", resp.status, log::ms(resp.total));
                    // Validate before caching: a 503 body must never replace a good cache.
                    let r = gm::routes_from_list(&v).filter(|r| !r.is_empty()).ok_or_else(|| format!("op=list HTTP {}: no challenges; cache left untouched", resp.status))?;
                    say!("routes", "cached at {}", gm::save_routes(&v)?.display());
                    r
                }
            };
            let filter = pos.first().map(String::as_str).unwrap_or("");
            for r in routes.iter().filter(|r| r.id.contains(filter)) {
                println!(
                    "{:<70} rft={:<5} deadline={:>7} ranked={:>4} regular={:>4} sets={}/{} {}{}",
                    r.id,
                    r.run_for_time_supported,
                    r.ranked_deadline_ms.map(|d| d.to_string()).unwrap_or("-".into()),
                    r.ranked_pool_size.map(|d| d.to_string()).unwrap_or("-".into()),
                    r.regular_pool_size.map(|d| d.to_string()).unwrap_or("-".into()),
                    r.ranked_set_count.map(|d| d.to_string()).unwrap_or("-".into()),
                    r.regular_set_count.map(|d| d.to_string()).unwrap_or("-".into()),
                    if r.solved { "solved" } else { "" },
                    if r.one_shot() { " ONE-SHOT" } else { "" },
                );
            }
            Ok(true)
        }
        _ => {
            o.challenge = pos.first().ok_or(USAGE)?.clone() + suffix;
            let pool = Pool::new(endpoint(url)?);
            run::attempt(&o, &pool)
        }
    }
}

fn main() -> ExitCode {
    match real_main() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
