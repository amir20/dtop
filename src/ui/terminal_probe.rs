//! Asks the terminal about itself once at startup, in one round trip: its name
//! (XTVERSION, for the icon style) and its background color (OSC 11, so the
//! selection highlight stays readable on light themes). DA1 goes last as a
//! sentinel: every terminal answers it and replies come back in order, so a
//! terminal that ignores the other queries costs one round trip, not the timeout.
//!
//! The queries travel in-band, so this works when dtop runs over SSH. Must run in
//! raw mode, before the keyboard worker reads stdin, or the replies are either
//! echoed or eaten as keystrokes.

/// How long to wait for the terminal's replies. One round trip over SSH can
/// take 200ms+, and only terminals that answer none of the queries hit this.
pub(crate) const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

/// What the terminal said about itself; each field is `None` if it didn't answer
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TerminalReply {
    /// XTVERSION name, e.g. `ghostty 1.3.2` (under tmux, tmux's own)
    pub name: Option<String>,
    /// Default background color
    pub background: Option<(u8, u8, u8)>,
}

/// Probes the terminal. A reply that arrives after the timeout (a link with a
/// round trip over 1s) reaches the keyboard worker as keystrokes; terminals that
/// don't support these queries never answer at all, so only slow links see it.
#[cfg(unix)]
pub fn probe() -> TerminalReply {
    use std::io::{IsTerminal, Read, Write};
    use std::os::fd::AsRawFd;

    let mut stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    if !stdin.is_terminal() || !stdout.is_terminal() {
        return TerminalReply::default();
    }
    if stdout
        .write_all(b"\x1b[>0q\x1b]11;?\x1b\\\x1b[c")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return TerminalReply::default();
    }

    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 256];
    while !has_da1_reply(&buf) {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let mut fds = libc::pollfd {
            fd: stdin.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd, and the count matches.
        let ready = unsafe { libc::poll(&mut fds, 1, remaining.as_millis() as libc::c_int) };
        if ready < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            // A signal (e.g. SIGWINCH from a resize) is not the end of the reply
            continue;
        }
        if ready <= 0 {
            break;
        }
        match stdin.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    parse_replies(&buf)
}

#[cfg(not(unix))]
pub fn probe() -> TerminalReply {
    TerminalReply::default()
}

fn parse_replies(buf: &[u8]) -> TerminalReply {
    TerminalReply {
        name: parse_xtversion(buf),
        background: parse_osc11(buf),
    }
}

/// Extracts the terminal name from an XTVERSION reply (`ESC P > | name ESC \`).
fn parse_xtversion(buf: &[u8]) -> Option<String> {
    let start = buf.windows(4).position(|w| w == b"\x1bP>|")? + 4;
    let rest = &buf[start..];
    let end = rest.iter().position(|&b| b == 0x1b || b == 0x07)?;
    Some(String::from_utf8_lossy(&rest[..end]).into_owned())
}

/// Extracts the color from an OSC 11 reply (`ESC ] 11 ; rgb:RRRR/GGGG/BBBB ST`).
/// Each component has 1 to 4 hex digits, scaled here to 8 bits.
fn parse_osc11(buf: &[u8]) -> Option<(u8, u8, u8)> {
    let start = buf.windows(9).position(|w| w == b"\x1b]11;rgb:")? + 9;
    let rest = &buf[start..];
    let end = rest.iter().position(|&b| b == 0x1b || b == 0x07)?;
    let text = std::str::from_utf8(&rest[..end]).ok()?;
    let mut parts = text.split('/').map(|hex| {
        if hex.is_empty() || hex.len() > 4 {
            return None;
        }
        let value = u32::from_str_radix(hex, 16).ok()?;
        let max = (1u32 << (4 * hex.len())) - 1;
        Some((value * 255 / max) as u8)
    });
    let color = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(color)
}

/// Whether `buf` ends with a primary device attributes reply (`ESC [ ? ... c`).
/// Every terminal answers DA1, and replies come back in order, so once it is
/// here there is no other reply still on its way.
fn has_da1_reply(buf: &[u8]) -> bool {
    buf.windows(3)
        .rposition(|w| w == b"\x1b[?")
        .is_some_and(|i| buf[i + 3..].contains(&b'c'))
}

/// Whether a background color is light, by relative luminance
pub fn is_light((r, g, b): (u8, u8, u8)) -> bool {
    let luminance = 0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b);
    luminance > 127.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_xtversion() {
        // Captured from Ghostty over SSH: XTVERSION reply, then DA1.
        let reply = b"\x1bP>|ghostty 1.3.2-main+b1d9492cf\x1b\\\x1b[?62;22;52c";
        assert!(has_da1_reply(reply));
        assert_eq!(
            parse_xtversion(reply).as_deref(),
            Some("ghostty 1.3.2-main+b1d9492cf")
        );

        // A terminal that ignores XTVERSION answers DA1 alone.
        let da1_only = b"\x1b[?1;2c";
        assert!(has_da1_reply(da1_only));
        assert_eq!(parse_xtversion(da1_only), None);

        // XTVERSION arrived but DA1 hasn't yet: keep reading.
        assert!(!has_da1_reply(b"\x1bP>|WezTerm 20240203\x1b\\"));
        assert!(!has_da1_reply(b"\x1bP>|WezTerm 20240203\x1b\\\x1b[?62;2"));
    }

    #[test]
    fn test_parse_all_three_replies() {
        // Captured from Rex with a light theme set
        let reply =
            b"\x1bP>|rex 0.0.1793645657-gd018d2\x1b\\\x1b]11;rgb:ffff/ffff/ffff\x1b\\\x1b[?62;22c";
        assert_eq!(
            parse_replies(reply),
            TerminalReply {
                name: Some("rex 0.0.1793645657-gd018d2".to_string()),
                background: Some((255, 255, 255)),
            }
        );
    }

    #[test]
    fn test_parse_osc11() {
        assert_eq!(
            parse_osc11(b"\x1b]11;rgb:1e1e/1e1e/1e1e\x07"),
            Some((30, 30, 30))
        );
        // Fewer digits per component, BEL terminator
        assert_eq!(
            parse_osc11(b"\x1b]11;rgb:f/80/fff\x07"),
            Some((255, 128, 255))
        );
        assert_eq!(parse_osc11(b"\x1b]11;rgb:ffff/ffff\x07"), None);
        assert_eq!(parse_osc11(b"\x1b]11;rgb:ffff/ffff/ffff/ffff\x07"), None);
        assert_eq!(parse_osc11(b"\x1b]11;rgb:zz/00/00\x07"), None);
        // Cut off before the terminator
        assert_eq!(parse_osc11(b"\x1b]11;rgb:ffff/ffff/ffff"), None);
        assert_eq!(parse_osc11(b"\x1b[?62;22c"), None);
    }

    #[test]
    fn test_is_light() {
        assert!(is_light((255, 255, 255)));
        assert!(is_light((253, 246, 227))); // Solarized Light
        assert!(!is_light((30, 30, 30)));
        assert!(!is_light((0, 43, 54))); // Solarized Dark
    }
}
