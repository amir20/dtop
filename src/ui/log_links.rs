//! Finds URLs in a log line and where each lands once the line is wrapped, so
//! the log view can draw them as OSC 8 hyperlinks (`Hyperlink`) over the text.
//!
//! A URL that wrapping splits across rows becomes one piece per row, each
//! linking to the whole URL: a row on its own would otherwise hold a cut-off
//! address, and a link to the wrong place is worse than none.

use std::ops::Range;

use unicode_width::UnicodeWidthStr;

/// The part of a URL on one wrapped row
#[derive(Debug, PartialEq, Eq)]
pub struct LinkPiece {
    /// Row within the wrapped line, 0 for the first
    pub row: usize,
    /// Column the piece starts at, counting a continuation row's indent
    pub column: usize,
    /// Bytes of the line's text shown on this row
    pub text: Range<usize>,
    /// Bytes of the whole URL
    pub url: Range<usize>,
}

/// Byte ranges of the `http://` and `https://` URLs in `text`
pub fn find_urls(text: &str) -> Vec<Range<usize>> {
    let mut urls = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find("http") {
        let start = from + found;
        from = start + 4;
        let rest = &text[start..];
        let scheme = if rest.starts_with("https://") {
            8
        } else if rest.starts_with("http://") {
            7
        } else {
            continue;
        };
        // Not the tail of another word, as in "xhttp://"
        if text[..start]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
        {
            continue;
        }
        let len = rest
            .find(|c: char| c.is_whitespace() || c.is_control() || "\"'`<>|\\^{}".contains(c))
            .unwrap_or(rest.len());
        let mut url = &rest[..len];
        // Punctuation that ends the sentence or wraps the URL, not the URL itself
        loop {
            let trimmed = url.trim_end_matches(['.', ',', ';', ':', '!', '?']);
            let trimmed = match trimmed.chars().next_back() {
                Some(')') if !trimmed.contains('(') => &trimmed[..trimmed.len() - 1],
                Some(']') if !trimmed.contains('[') => &trimmed[..trimmed.len() - 1],
                _ => trimmed,
            };
            if trimmed.len() == url.len() {
                break;
            }
            url = trimmed;
        }
        if url.len() > scheme {
            urls.push(start..start + url.len());
            from = start + url.len();
        }
    }
    urls
}

/// Where each URL in `text` is drawn, given the byte offsets its wrapped rows
/// start at (`breaks`, from `log_wrap::wrap_breaks`) and their `indent`
pub fn link_pieces(text: &str, breaks: &[usize], indent: usize) -> Vec<LinkPiece> {
    let mut pieces = Vec::new();
    for url in find_urls(text) {
        for row in 0..=breaks.len() {
            let row_start = if row == 0 { 0 } else { breaks[row - 1] };
            let row_end = breaks.get(row).copied().unwrap_or(text.len());
            let start = url.start.max(row_start);
            let end = url.end.min(row_end);
            if start >= end {
                continue;
            }
            let row_indent = if row == 0 { 0 } else { indent };
            pieces.push(LinkPiece {
                row,
                column: row_indent + text[row_start..start].width(),
                text: start..end,
                url: url.clone(),
            });
        }
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls(text: &str) -> Vec<&str> {
        find_urls(text).into_iter().map(|r| &text[r]).collect()
    }

    #[test]
    fn finds_urls_and_leaves_the_punctuation_around_them() {
        assert_eq!(
            urls("GET https://example.com/a?b=1&c=2 200"),
            ["https://example.com/a?b=1&c=2"]
        );
        assert_eq!(
            urls("see http://a.io/x, (https://b.io/y). Done"),
            ["http://a.io/x", "https://b.io/y"]
        );
        assert_eq!(
            urls("url=\"https://example.com/path\" ok"),
            ["https://example.com/path"]
        );
        assert_eq!(
            urls("at https://en.wikipedia.org/wiki/Rust_(language)."),
            ["https://en.wikipedia.org/wiki/Rust_(language)"]
        );
        assert_eq!(urls("serving url=http://[::1]:80"), ["http://[::1]:80"]);
        assert_eq!(
            urls("{'url': 'https://x.io/v1/ping', 'p': {}}"),
            ["https://x.io/v1/ping"]
        );
    }

    #[test]
    fn ignores_what_is_not_a_url() {
        assert!(urls("http status 200, https enabled").is_empty());
        assert!(urls("scheme only: https:// and http://.").is_empty());
        assert!(urls("xhttp://example.com").is_empty());
        assert!(urls("").is_empty());
    }

    #[test]
    fn a_url_on_one_row_is_one_piece() {
        let text = "12:00 GET https://a.io/x ok";
        let pieces = link_pieces(text, &[], 6);
        assert_eq!(
            pieces,
            [LinkPiece {
                row: 0,
                column: 10,
                text: 10..24,
                url: 10..24
            }]
        );
    }

    /// Wrapping cut the URL in two: both rows link to the whole of it
    #[test]
    fn a_wrapped_url_links_whole_from_every_row() {
        let text = "12:00 https://example.com/a/very/long/path end";
        // Rows: "12:00 https://example" | "      .com/a/very/long" | "      /path end"
        let breaks = [21, 37];
        let pieces = link_pieces(text, &breaks, 6);
        let shown: Vec<(usize, usize, &str)> = pieces
            .iter()
            .map(|p| (p.row, p.column, &text[p.text.clone()]))
            .collect();
        assert_eq!(
            shown,
            [
                (0, 6, "https://example"),
                (1, 6, ".com/a/very/long"),
                (2, 6, "/path")
            ]
        );
        for piece in &pieces {
            assert_eq!(
                &text[piece.url.clone()],
                "https://example.com/a/very/long/path"
            );
        }
    }

    #[test]
    fn columns_count_display_width() {
        let text = "日本 https://a.io";
        assert_eq!(link_pieces(text, &[], 0)[0].column, 5);
    }
}
