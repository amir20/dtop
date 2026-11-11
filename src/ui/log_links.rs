//! OSC 8 hyperlinks for URLs in the log view.
//!
//! Two halves. `link_pieces` finds the URLs in a log line and where each lands
//! once the line is wrapped. A URL that wrapping splits across rows becomes one
//! piece per row, each linking to the whole URL: a row on its own would
//! otherwise hold a cut-off address, and a link to the wrong place is worse
//! than none.
//!
//! `LinkWriter` then tells the terminal about them, outside ratatui's buffer.
//! The container list puts the escape sequences into cells (`Hyperlink`), which
//! is fine for names that rarely move. Log lines scroll, and ratatui only
//! rewrites the cells whose content changed: an unchanged cell would keep the
//! link it had on the previous frame (an older URL, or a link on text that is
//! no longer one), and an opener whose closing cell wasn't rewritten would leak
//! onto whatever is written next. So after each draw the writer re-prints every
//! link's cells between an opener and a closer, and re-prints, plain, the cells
//! that were linked on the previous frame and no longer are.

use std::io::{self, Write};
use std::ops::Range;

use crossterm::cursor::{RestorePosition, SavePosition};
use crossterm::queue;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::buffer::{Buffer, Cell};
use unicode_width::UnicodeWidthStr;

use super::hyperlinks::{OSC8_PREFIX, ST, is_safe_url};

/// A URL, or the part of one on a row, as drawn by the log view this frame
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLink {
    pub x: u16,
    pub y: u16,
    /// The text in those cells
    pub label: String,
    pub url: String,
}

/// Cells to re-print, as a link to `url` or (None) as plain text
#[derive(Debug, PartialEq)]
pub struct LinkWrite {
    pub url: Option<String>,
    pub cells: Vec<(u16, u16, Cell)>,
}

/// Remembers which cells the terminal holds as links, so they can be cleared
#[derive(Default)]
pub struct LinkWriter {
    /// (x, y, width) of every link written on the last frame
    linked: Vec<(u16, u16, u16)>,
}

impl LinkWriter {
    /// Forgets what was written: the screen was cleared (e.g. after a shell
    /// session), so no cell holds a link any more
    pub fn forget(&mut self) {
        self.linked.clear();
    }

    /// What to write so the terminal's links match `links`, given the frame
    /// just drawn. A link whose cells no longer show its label (a popup or a
    /// toast is drawn over it) is left out, and so cleared.
    pub fn plan(&mut self, buffer: &Buffer, links: &[LogLink]) -> Vec<LinkWrite> {
        let cells = |x: u16, y: u16, width: u16| -> Vec<(u16, u16, Cell)> {
            (x..x.saturating_add(width))
                .filter_map(|x| buffer.cell((x, y)).map(|cell| (x, y, cell.clone())))
                .collect()
        };

        let mut writes = Vec::new();
        let mut linked = Vec::with_capacity(links.len());
        for link in links {
            let width = link.label.width() as u16;
            let shown = cells(link.x, link.y, width);
            let text: String = shown.iter().map(|(_, _, cell)| cell.symbol()).collect();
            if width == 0 || text != link.label || !is_safe_url(&link.url) {
                continue;
            }
            linked.push((link.x, link.y, width));
            writes.push(LinkWrite {
                url: Some(link.url.clone()),
                cells: shown,
            });
        }

        // Cells linked last frame that aren't part of a link now
        let still_linked = |x: u16, y: u16| {
            linked
                .iter()
                .any(|&(lx, ly, width)| ly == y && (lx..lx + width).contains(&x))
        };
        let stale: Vec<(u16, u16, Cell)> = self
            .linked
            .iter()
            .flat_map(|&(x, y, width)| cells(x, y, width))
            .filter(|&(x, y, _)| !still_linked(x, y))
            .collect();
        if !stale.is_empty() {
            // Before the links, so a cell both lists name ends up linked
            writes.insert(
                0,
                LinkWrite {
                    url: None,
                    cells: stale,
                },
            );
        }

        self.linked = linked;
        writes
    }
}

/// Carries out a `LinkWriter::plan`, leaving the cursor where it was
pub fn write_links<W: Write>(
    backend: &mut CrosstermBackend<W>,
    writes: &[LinkWrite],
) -> io::Result<()> {
    if writes.is_empty() {
        return Ok(());
    }
    queue!(backend, SavePosition)?;
    for write in writes {
        if let Some(url) = &write.url {
            write!(backend, "{OSC8_PREFIX}{url}{ST}")?;
        }
        let cells = write.cells.iter().map(|(x, y, cell)| (*x, *y, cell));
        let drawn = backend.draw(cells);
        // Close the link even if the draw failed, or it leaks onto the screen
        if write.url.is_some() {
            write!(backend, "{OSC8_PREFIX}{ST}")?;
        }
        drawn?;
    }
    queue!(backend, RestorePosition)?;
    Ok(())
}

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

    fn buffer(rows: &[&str]) -> Buffer {
        Buffer::with_lines(rows.iter().copied())
    }

    fn link(x: u16, y: u16, label: &str, url: &str) -> LogLink {
        LogLink {
            x,
            y,
            label: label.to_string(),
            url: url.to_string(),
        }
    }

    fn text(write: &LinkWrite) -> String {
        write.cells.iter().map(|(_, _, c)| c.symbol()).collect()
    }

    /// Every frame re-prints each link, and a link that is gone (the line
    /// scrolled, here) has its old cells re-printed plain
    #[test]
    fn writer_relinks_every_frame_and_clears_what_scrolled_away() {
        let mut writer = LinkWriter::default();
        let url = "https://a.io/x";

        let first = buffer(&["GET https://a.io/x ok", "plain line          "]);
        let writes = writer.plan(&first, &[link(4, 0, url, url)]);
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].url.as_deref(), Some(url));
        assert_eq!(text(&writes[0]), url);

        // Unchanged frame: linked again, nothing to clear
        let writes = writer.plan(&first, &[link(4, 0, url, url)]);
        assert_eq!(writes.len(), 1);
        assert!(writes[0].url.is_some());

        // The line moved up and out; row 0 now shows other text
        let second = buffer(&["plain line          ", "another line        "]);
        let writes = writer.plan(&second, &[]);
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].url, None);
        assert_eq!(text(&writes[0]), "n line        ");
        assert_eq!(writes[0].cells[0].0, 4);

        assert!(writer.plan(&second, &[]).is_empty());
    }

    /// A toast drawn over a link: the cells no longer show the URL, so they
    /// are not linked, and what was linked is cleared
    #[test]
    fn writer_drops_a_link_that_is_drawn_over() {
        let mut writer = LinkWriter::default();
        let url = "https://a.io/x";
        writer.plan(&buffer(&["GET https://a.io/x ok"]), &[link(4, 0, url, url)]);

        let covered = buffer(&["GET https│Copied│ ok"]);
        let writes = writer.plan(&covered, &[link(4, 0, url, url)]);
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].url, None);
    }

    #[test]
    fn writer_refuses_a_url_with_control_characters() {
        let mut writer = LinkWriter::default();
        let writes = writer.plan(
            &buffer(&["GET https://a.io/x ok"]),
            &[link(4, 0, "https://a.io/x", "https://a.io/\x1b]8;;evil")],
        );
        assert!(writes.is_empty());
    }

    /// A writer the test can read back after the backend took ownership
    #[derive(Clone, Default)]
    struct Captured(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// The bytes: opener, the cells, closer, with the cursor put back
    #[test]
    fn links_are_written_between_an_opener_and_a_closer() {
        let mut writer = LinkWriter::default();
        let url = "https://a.io/x";
        let writes = writer.plan(&buffer(&["GET https://a.io/x ok"]), &[link(4, 0, url, url)]);

        let captured = Captured::default();
        let mut backend = CrosstermBackend::new(captured.clone());
        write_links(&mut backend, &writes).unwrap();
        let out = String::from_utf8(captured.0.borrow().clone()).unwrap();

        let opener = "\x1b]8;;https://a.io/x\x1b\\";
        let closer = "\x1b]8;;\x1b\\";
        let opened = out.find(opener).expect("an opener") + opener.len();
        let closed = out.rfind(closer).expect("a closer");
        // Between them: the label, and only cursor and style sequences besides
        let mut label = String::new();
        let mut chars = out[opened..closed].chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                chars.by_ref().find(char::is_ascii_alphabetic);
            } else {
                label.push(c);
            }
        }
        assert_eq!(label, url, "{out:?}");
        assert!(
            out.starts_with("\x1b7") && out.ends_with("\x1b8"),
            "{out:?}"
        );

        let captured = Captured::default();
        write_links(&mut CrosstermBackend::new(captured.clone()), &[]).unwrap();
        assert!(captured.0.borrow().is_empty());
    }
}
