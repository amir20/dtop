//! Finds a timestamp an app printed at the start of a log line that repeats the
//! one Docker recorded for it, so the log view can show just one of them.
//!
//! Ported from Dozzle (`internal/container/logparse/timestamp_prefix.go`). Only
//! ISO 8601 / RFC 3339 shapes are read, and only a timestamp that agrees with
//! Docker's is reported: a line's other clocks (an event time, another zone's
//! wall time that isn't a whole offset away) are left alone.

use chrono::NaiveDate;

/// A zoned timestamp is a duplicate when it is this close to Docker's.
/// Loggers often truncate to the second, so the text can trail by up to 1s.
const TOLERANCE_MS: i64 = 2_000;
/// A timestamp without a zone is assumed to be local time, so it may be off
/// from Docker's UTC time by a whole number of quarter hours, up to this far.
const MAX_OFFSET_MS: i64 = 14 * 3_600_000;
const QUARTER_HOUR_MS: i64 = 15 * 60_000;

/// Returns how many leading bytes of `message` are a timestamp that repeats
/// `docker_ms` (unix milliseconds), or 0 when there is none.
///
/// The prefix covers the timestamp's own ANSI coloring, an optional pair of
/// brackets and the whitespace after it, so `message[n..]` starts at the first
/// real character of the line. Everything it can match is ASCII.
///
/// This runs on every log line, so it parses by hand and bails out on the
/// first byte for lines that cannot start with a timestamp.
pub fn timestamp_prefix_len(message: &str, docker_ms: i64) -> usize {
    let s = message.as_bytes();
    match s.first() {
        Some(c) if c.is_ascii_digit() || *c == b'[' || *c == 0x1b => {}
        _ => return 0,
    }

    let mut colored = false;
    let Some(mut i) = skip_ansi(s, 0, &mut colored) else {
        return 0;
    };
    let bracketed = s.get(i) == Some(&b'[');
    if bracketed {
        let Some(next) = skip_ansi(s, i + 1, &mut colored) else {
            return 0;
        };
        i = next;
    }

    let Some((ts, zoned, end)) = parse_timestamp(s, i) else {
        return 0;
    };

    // Only resets may follow: the color that was set for the timestamp has to be
    // turned off before the text, or cutting it away would uncolor the rest.
    let mut reset = false;
    i = skip_resets(s, end, &mut reset);
    if bracketed {
        if s.get(i) != Some(&b']') {
            return 0;
        }
        i = skip_resets(s, i + 1, &mut reset);
    }
    if colored && !reset {
        return 0;
    }

    let before_spaces = i;
    while matches!(s.get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    if (!bracketed && i == before_spaces) || i >= s.len() {
        return 0;
    }

    if matches_docker_time(ts, zoned, docker_ms) {
        i
    } else {
        0
    }
}

fn matches_docker_time(ts: i64, zoned: bool, docker_ms: i64) -> bool {
    let diff = ts - docker_ms;
    if zoned {
        return diff.abs() <= TOLERANCE_MS;
    }
    if diff.abs() > MAX_OFFSET_MS + TOLERANCE_MS {
        return false;
    }
    let nearest = (diff + diff.signum() * QUARTER_HOUR_MS / 2) / QUARTER_HOUR_MS * QUARTER_HOUR_MS;
    (diff - nearest).abs() <= TOLERANCE_MS
}

/// Consumes SGR escape sequences (`ESC [ ... m`) starting at `i`. A non-SGR
/// escape is not something we know how to drop safely, so it fails.
fn skip_ansi(s: &[u8], mut i: usize, colored: &mut bool) -> Option<usize> {
    while s.get(i) == Some(&0x1b) {
        let (end, is_reset) = read_sgr(s, i)?;
        if !is_reset {
            *colored = true;
        }
        i = end;
    }
    Some(i)
}

/// Consumes only SGR sequences that turn attributes off.
fn skip_resets(s: &[u8], mut i: usize, reset: &mut bool) -> usize {
    while s.get(i) == Some(&0x1b) {
        match read_sgr(s, i) {
            Some((end, true)) => {
                *reset = true;
                i = end;
            }
            _ => break,
        }
    }
    i
}

/// Reads one `ESC [ params m` sequence at `i`, returning its end and whether
/// every parameter only turns something off (0, 22, 23, 24, 39, 49 or empty).
fn read_sgr(s: &[u8], i: usize) -> Option<(usize, bool)> {
    if s.get(i + 1) != Some(&b'[') {
        return None;
    }
    let mut is_reset = true;
    let mut param = 0u32;
    for (j, &c) in s.iter().enumerate().skip(i + 2).take(30) {
        match c {
            b'0'..=b'9' => param = param * 10 + u32::from(c - b'0'),
            b';' | b'm' => {
                if !matches!(param, 0 | 22 | 23 | 24 | 39 | 49) {
                    is_reset = false;
                }
                param = 0;
                if c == b'm' {
                    return Some((j + 1, is_reset));
                }
            }
            _ => return None,
        }
    }
    None
}

/// Reads a date and time at `s[i..]` in one of these shapes:
///
/// ```text
/// 2006-01-02T15:04:05Z
/// 2006-01-02 15:04:05.000000+07:00
/// 2006-01-02 15:04:05,000
/// 2006/01/02 15:04:05
/// 2006-01-02 15:04:05 +0700
/// 2006-01-02 15:04:05.000 UTC
/// ```
///
/// Returns unix milliseconds (treating a zoneless time as UTC), whether a zone
/// was present, and the index after the timestamp.
fn parse_timestamp(s: &[u8], i: usize) -> Option<(i64, bool, usize)> {
    // yyyy-mm-dd hh:mm:ss is 19 bytes
    if s.len() < i + 19 {
        return None;
    }
    let year = digits(s, i, 4)?;
    let sep = s[i + 4];
    let month = digits(s, i + 5, 2)?;
    let day = digits(s, i + 8, 2)?;
    let hour = digits(s, i + 11, 2)?;
    let minute = digits(s, i + 14, 2)?;
    let second = digits(s, i + 17, 2)?;
    if (sep != b'-' && sep != b'/')
        || s[i + 7] != sep
        || (s[i + 10] != b'T' && s[i + 10] != b' ')
        || s[i + 13] != b':'
        || s[i + 16] != b':'
    {
        return None;
    }

    let mut j = i + 19;
    let mut millis = 0i64;
    if matches!(s.get(j), Some(b'.' | b',')) {
        let start = j + 1;
        let mut k = start;
        while s.get(k).is_some_and(u8::is_ascii_digit) {
            if k - start < 3 {
                millis += i64::from(s[k] - b'0') * [100, 10, 1][k - start];
            }
            k += 1;
        }
        if k == start {
            return None;
        }
        j = k;
    }

    let mut zoned = false;
    let mut offset_secs = 0i64;
    match s.get(j) {
        Some(b'Z') => {
            zoned = true;
            j += 1;
        }
        Some(b'+' | b'-') => {
            if let Some((secs, end)) = parse_offset(s, j) {
                (offset_secs, zoned, j) = (secs, true, end);
            }
        }
        // gunicorn's "15:26:03 +0200" and postgres' "10:02:07.048 UTC"
        Some(b' ') => {
            if let Some((secs, end)) = parse_offset(s, j + 1) {
                (offset_secs, zoned, j) = (secs, true, end);
            } else if matches!(s.get(j + 1..j + 5), Some(b"UTC " | b"GMT ")) {
                (zoned, j) = (true, j + 4);
            }
        }
        _ => {}
    }

    // A leap second reads as the second before it; the tolerance covers it
    let time = NaiveDate::from_ymd_opt(year as i32, month, day)?.and_hms_opt(
        hour,
        minute,
        second.min(59),
    )?;
    let ms = time.and_utc().timestamp_millis() + millis - offset_secs * 1000;
    Some((ms, zoned, j))
}

/// Reads `+07:00` or `+0700` at `j`, returning the offset in seconds and its end.
fn parse_offset(s: &[u8], j: usize) -> Option<(i64, usize)> {
    let negative = match s.get(j)? {
        b'+' => false,
        b'-' => true,
        _ => return None,
    };
    let hours = digits(s, j + 1, 2)?;
    let mut n = j + 3;
    if s.get(n) == Some(&b':') {
        n += 1;
    }
    let minutes = digits(s, n, 2)?;
    if hours > 14 || minutes > 59 {
        return None;
    }
    let secs = i64::from(hours * 3600 + minutes * 60);
    Some((if negative { -secs } else { secs }, n + 2))
}

fn digits(s: &[u8], i: usize, n: usize) -> Option<u32> {
    let slice = s.get(i..i + n)?;
    slice.iter().try_fold(0u32, |v, c| {
        c.is_ascii_digit().then(|| v * 10 + u32::from(c - b'0'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    /// Docker's timestamp for every case: 2026-09-13T22:28:56.412Z
    fn docker_ms() -> i64 {
        DateTime::parse_from_rfc3339("2026-09-13T22:28:56.412Z")
            .unwrap()
            .timestamp_millis()
    }

    fn rest(line: &str) -> &str {
        &line[timestamp_prefix_len(line, docker_ms())..]
    }

    #[test]
    fn strips_a_timestamp_that_repeats_dockers() {
        let cases = [
            (
                "zerolog colored",
                "\x1b[90m2026-09-13T22:28:56Z\x1b[0m \x1b[32mINF\x1b[0m Starting server",
                "\x1b[32mINF\x1b[0m Starting server",
            ),
            (
                "plain RFC3339",
                "2026-09-13T22:28:56Z INF Starting server",
                "INF Starting server",
            ),
            (
                "RFC3339Nano",
                "2026-09-13T22:28:56.412345678Z hello",
                "hello",
            ),
            (
                "space and comma fraction",
                "2026-09-13 22:28:56,412 INFO hello",
                "INFO hello",
            ),
            (
                "bracketed",
                "[2026-09-13 22:28:56.412] [info] hello",
                "[info] hello",
            ),
            (
                "bracketed no space",
                "[2026-09-13T22:28:56Z]INFO hello",
                "INFO hello",
            ),
            (
                "slashes",
                "2026/09/13 22:28:56 [error] nginx",
                "[error] nginx",
            ),
            (
                "offset with colon",
                "2026-09-13T15:28:56-07:00 hello",
                "hello",
            ),
            (
                "offset without colon",
                "2026-09-14T03:58:56.1+0530 hello",
                "hello",
            ),
            ("local time no zone", "2026-09-13 18:28:56 hello", "hello"),
            (
                "local time half hour zone",
                "2026-09-14 03:58:57 hello",
                "hello",
            ),
            ("tab after", "2026-09-13T22:28:56Z\thello", "hello"),
            (
                "reset inside bracket",
                "\x1b[2m[2026-09-13T22:28:56Z\x1b[0m] hello",
                "hello",
            ),
            (
                "gunicorn: space before the offset",
                "[2026-09-14 00:28:56 +0200] [7] [INFO] Successful login",
                "[7] [INFO] Successful login",
            ),
            (
                "postgres: UTC after the time",
                "2026-09-13 22:28:56.048 UTC [24] LOG:  shutting down",
                "[24] LOG:  shutting down",
            ),
        ];
        for (name, line, want) in cases {
            assert_eq!(rest(line), want, "{name}");
        }
    }

    #[test]
    fn leaves_everything_else_alone() {
        let cases = [
            (
                "mismatched zoned time",
                "2026-09-13T22:20:00Z INF old event",
            ),
            ("zoned offset disagrees", "2026-09-13T22:28:56+01:00 hello"),
            (
                "local time not on a quarter hour",
                "2026-09-13 18:21:56 hello",
            ),
            ("local time too far off", "2026-09-14 14:28:56 hello"),
            ("starts with digits", "200 OK GET /index.html"),
            ("date only", "2026-09-13 hello"),
            (
                "color runs past timestamp",
                "\x1b[31m2026-09-13T22:28:56Z ERROR boom\x1b[0m",
            ),
            (
                "color set again after reset is kept",
                "\x1b[90m2026-09-13T22:28:56Z\x1b[0m\x1b[31m ERR",
            ),
            ("nothing after timestamp", "2026-09-13T22:28:56Z   "),
            ("glued to text", "2026-09-13T22:28:56Zhello"),
            ("unclosed bracket", "[2026-09-13T22:28:56Z hello"),
            ("invalid month", "2026-13-13T22:28:56Z hello"),
            ("invalid day", "2026-02-31T22:28:56Z hello"),
            ("UTC that disagrees", "2026-09-13 18:28:56 UTC hello"),
            ("empty", ""),
            ("non-ASCII start", "é2026-09-13T22:28:56Z hello"),
            // From the samples in issue #290
            (
                "celery: text inside the bracket",
                "[2026-09-13 22:28:56,867: INFO/MainProcess] ready",
            ),
            (
                "jetty: glued to the level",
                "2026-09-13 22:28:56.073:INFO:oejs.x:jetty",
            ),
            ("time only", "[22:28:56] [INF] FFmpeg exited with code 0"),
            (
                "syslog",
                "Sep 13 22:28:56 dnsmasq[6]: started, version 2.93",
            ),
        ];
        for (name, line) in cases {
            assert_eq!(timestamp_prefix_len(line, docker_ms()), 0, "{name}");
        }
    }

    /// Frigate prints two; only the one that repeats Docker's goes
    #[test]
    fn strips_only_the_first_of_two_timestamps() {
        assert_eq!(
            rest("2026-09-13 22:28:56.836556606  [2026-09-14 00:28:56] frigate.api INFO"),
            "[2026-09-14 00:28:56] frigate.api INFO"
        );
    }
}
