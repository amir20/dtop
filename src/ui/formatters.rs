//! Formatting utilities for displaying values in the UI

use chrono::Utc;
use std::fmt::Write;
use std::sync::LazyLock;
use timeago::Formatter;

static TIMEAGO_FORMATTER: LazyLock<Formatter> = LazyLock::new(Formatter::new);

const KB: f64 = 1024.0;
const MB: f64 = KB * 1024.0;
const GB: f64 = MB * 1024.0;

/// Writes a byte value with the appropriate unit into an existing buffer.
///
/// This is the allocation-free core used by both the `String`-returning
/// helpers and the hot-path renderers that format directly into a reused
/// buffer.
fn write_byte_value(
    buf: &mut String,
    value: f64,
    suffix: &str,
    include_b: bool,
    precisions: (usize, usize, usize, usize),
) {
    let (gb_prec, mb_prec, kb_prec, b_prec) = precisions;
    let b = if include_b { "B" } else { "" };

    // `write!` to a `String` is infallible, so each `let _` discards the Result.
    if value >= GB {
        let gb = value / GB;
        // When precision is 0, show one decimal for fractional values so
        // 1.5G doesn't render as 2G. Whole numbers stay clean (e.g. "4G").
        if gb_prec == 0 && (gb - gb.round()).abs() >= 0.05 {
            let _ = write!(buf, "{:.1}G{}{}", gb, b, suffix);
        } else {
            let _ = write!(buf, "{:.prec$}G{}{}", gb, b, suffix, prec = gb_prec);
        }
    } else if value >= MB {
        let _ = write!(buf, "{:.prec$}M{}{}", value / MB, b, suffix, prec = mb_prec);
    } else if value >= KB {
        let _ = write!(buf, "{:.prec$}K{}{}", value / KB, b, suffix, prec = kb_prec);
    } else {
        let _ = write!(buf, "{:.prec$}B{}", value, suffix, prec = b_prec);
    }
}

/// Writes a human-readable byte value (B, K, M, G) into an existing buffer.
pub fn write_bytes(buf: &mut String, bytes: u64) {
    write_byte_value(buf, bytes as f64, "", false, (0, 0, 0, 0));
}

/// Formats bytes into a human-readable string (B, K, M, G).
///
/// Production rendering uses `write_bytes` to format directly into a reused
/// buffer; this `String`-returning convenience wrapper is retained for tests.
#[cfg(test)]
pub fn format_bytes(bytes: u64) -> String {
    let mut s = String::new();
    write_bytes(&mut s, bytes);
    s
}

/// Formats bytes per second into a human-readable string (KB/s, MB/s, GB/s)
pub fn format_bytes_per_sec(bytes_per_sec: f64) -> String {
    let mut s = String::new();
    write_byte_value(&mut s, bytes_per_sec, "/s", true, (2, 2, 1, 0));
    s
}

/// Formats the time elapsed since container creation
pub fn format_time_elapsed(created: Option<&chrono::DateTime<Utc>>) -> String {
    match created {
        Some(created_time) => {
            let now = Utc::now();
            TIMEAGO_FORMATTER.convert_chrono(*created_time, now)
        }
        None => "Unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_zero() {
        assert_eq!(format_bytes(0), "0B");
    }

    #[test]
    fn test_format_bytes_bytes() {
        assert_eq!(format_bytes(1), "1B");
        assert_eq!(format_bytes(512), "512B");
        assert_eq!(format_bytes(1023), "1023B");
    }

    #[test]
    fn test_format_bytes_kilobytes() {
        assert_eq!(format_bytes(1024), "1K");
        assert_eq!(format_bytes(1536), "2K"); // 1.5KB rounds to 2K
        assert_eq!(format_bytes(10240), "10K");
        assert_eq!(format_bytes(1048575), "1024K"); // Just under 1MB
    }

    #[test]
    fn test_format_bytes_megabytes() {
        assert_eq!(format_bytes(1048576), "1M"); // Exactly 1MB
        assert_eq!(format_bytes(536870912), "512M");
        assert_eq!(format_bytes(1073741823), "1024M"); // Just under 1GB
    }

    #[test]
    fn test_format_bytes_gigabytes() {
        assert_eq!(format_bytes(1073741824), "1G"); // Exactly 1GB
        assert_eq!(format_bytes(4294967296), "4G"); // 4GB
        assert_eq!(format_bytes(17179869184), "16G"); // 16GB
    }

    #[test]
    fn test_format_bytes_fractional_gigabytes() {
        assert_eq!(format_bytes(1610612736), "1.5G"); // 1.5GB
        assert_eq!(format_bytes(2684354560), "2.5G"); // 2.5GB
        assert_eq!(format_bytes(11274289152), "10.5G"); // 10.5GB
        // 1500 MiB = ~1.46 GiB
        assert_eq!(format_bytes(1500 * 1024 * 1024), "1.5G");
    }

    #[test]
    fn test_format_bytes_per_sec() {
        assert_eq!(format_bytes_per_sec(0.0), "0B/s");
        assert_eq!(format_bytes_per_sec(512.0), "512B/s");
        assert_eq!(format_bytes_per_sec(1024.0), "1.0KB/s");
        assert_eq!(format_bytes_per_sec(1048576.0), "1.00MB/s");
        assert_eq!(format_bytes_per_sec(1073741824.0), "1.00GB/s");
    }
}
