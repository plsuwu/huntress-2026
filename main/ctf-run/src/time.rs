//! UTC timestamps without dependencies: ISO-8601 formatting/parsing of `SystemTime` (millisecond
//! precision), enough to line client-side events up with the server's `started_at` / `deadline`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + (m <= 2) as i64, m, d)
}

/// Milliseconds since the Unix epoch.
pub fn unix_ms(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn now_ms() -> i64 {
    unix_ms(SystemTime::now())
}

/// `2026-10-14T18:05:31.519Z`
pub fn iso(ms: i64) -> String {
    let (days, rem) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3_600_000,
        rem / 60_000 % 60,
        rem / 1000 % 60,
        rem % 1000
    )
}

/// Parses `YYYY-MM-DDTHH:MM:SS[.fff]Z` into Unix milliseconds.
pub fn parse_iso(s: &str) -> Option<i64> {
    let s = s.strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-').map(|x| x.parse::<i64>());
    let (y, mo, da) = (d.next()?.ok()?, d.next()?.ok()?, d.next()?.ok()?);
    let (hms, frac) = time.split_once('.').unwrap_or((time, "0"));
    let mut t = hms.split(':').map(|x| x.parse::<i64>());
    let (h, mi, se) = (t.next()?.ok()?, t.next()?.ok()?, t.next()?.ok()?);
    let frac: String = frac.chars().chain("000".chars()).take(3).collect();
    let ms = frac.parse::<i64>().ok()?;
    Some(((days_from_civil(y, mo, da) * 24 + h) * 60 + mi) * 60_000 + se * 1000 + ms)
}

/// Parses an HTTP `Date` header (`Wed, 14 Oct 2026 17:28:50 GMT`) into Unix milliseconds.
pub fn parse_http_date(s: &str) -> Option<i64> {
    let mut p = s.split_whitespace().skip(1);
    let d: i64 = p.next()?.parse().ok()?;
    let mon = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let name = p.next()?;
    let m = mon.iter().position(|x| *x == name)? as i64 + 1;
    let y: i64 = p.next()?.parse().ok()?;
    let mut t = p.next()?.split(':').map(|x| x.parse::<i64>());
    let (h, mi, se) = (t.next()?.ok()?, t.next()?.ok()?, t.next()?.ok()?);
    Some(((days_from_civil(y, m, d) * 24 + h) * 60 + mi) * 60_000 + se * 1000)
}

pub fn since(ms: i64) -> Duration {
    Duration::from_millis((now_ms() - ms).max(0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let s = "2026-10-14T18:05:31.519Z";
        let ms = parse_iso(s).unwrap();
        assert_eq!(iso(ms), s);
        assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso("2000-03-01T00:00:00.5Z"), Some(951_868_800_500));
        assert_eq!(parse_http_date("Wed, 14 Oct 2026 18:05:31 GMT"), Some(ms - 519));
    }
}
