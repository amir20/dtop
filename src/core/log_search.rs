//! `/` search in the log view: which lines hold the query, and drawing them
//! with the matches highlighted. Matching is on the line as it is shown
//! (timestamp included) and ignores ASCII case.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use tui_input::Input;

/// The log view's search. It lives with the log state, apart from the
/// container list's filter (`AppState::search_input`), which stays applied
/// while a log is open.
#[derive(Debug, Default)]
pub struct LogSearch {
    pub input: Input,
    /// Still typing the query; Enter ends it
    pub editing: bool,
    /// Entry `n`/`N` last jumped to
    pub current: Option<usize>,
    /// Lines that matched as they streamed in since the query was applied.
    /// Reported to the terminal (`program_status`) so it can flag the tab.
    pub new_matches: usize,
}

impl LogSearch {
    /// The query, lowercased for matching. Empty while nothing is typed.
    pub fn needle(&self) -> String {
        self.input.value().to_ascii_lowercase()
    }
}

/// Byte ranges of `needle` in the line's text. `needle` must be ASCII-lowercased.
fn match_ranges(line: &Line, needle: &str) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    let mut text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    // In place, so byte offsets still line up with the spans
    text.make_ascii_lowercase();
    text.match_indices(needle)
        .map(|(at, found)| (at, at + found.len()))
        .collect()
}

/// Whether `line` holds `needle` (ASCII-lowercased)
pub fn matches(line: &Line, needle: &str) -> bool {
    !match_ranges(line, needle).is_empty()
}

/// `line` with every match of `needle` drawn in `style`, or None without a match
pub fn highlight(line: &Line<'static>, needle: &str, style: Style) -> Option<Line<'static>> {
    let ranges = match_ranges(line, needle);
    if ranges.is_empty() {
        return None;
    }

    let mut spans = Vec::with_capacity(line.spans.len() + 2 * ranges.len());
    let mut ranges = ranges.into_iter().peekable();
    let mut offset = 0;
    for span in &line.spans {
        let content: &str = &span.content;
        let end = offset + content.len();
        let mut at = offset;
        while at < end {
            // Skip matches that ended before this point
            while ranges.peek().is_some_and(|&(_, to)| to <= at) {
                ranges.next();
            }
            let (piece_end, piece_style) = match ranges.peek() {
                Some(&(from, to)) if from <= at => (to.min(end), span.style.patch(style)),
                Some(&(from, _)) if from < end => (from, span.style),
                _ => (end, span.style),
            };
            spans.push(Span::styled(
                content[at - offset..piece_end - offset].to_string(),
                piece_style,
            ));
            at = piece_end;
        }
        offset = end;
    }
    Some(Line::from(spans))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    const HIT: Style = Style::new().bg(Color::Yellow);

    fn pieces(line: &Line) -> Vec<(String, bool)> {
        line.spans
            .iter()
            .map(|s| (s.content.to_string(), s.style.bg == Some(Color::Yellow)))
            .collect()
    }

    #[test]
    fn matching_ignores_ascii_case() {
        let line = Line::from("12:00 ERROR Connection refused");
        assert!(matches(&line, "error"));
        assert!(matches(&line, "connection ref"));
        assert!(!matches(&line, "timeout"));
        assert!(!matches(&line, ""));
    }

    #[test]
    fn highlights_every_match_and_keeps_the_text() {
        let line = Line::from("an error, then another Error");
        let shown = highlight(&line, "error", HIT).unwrap();
        assert_eq!(
            pieces(&shown),
            [
                ("an ".to_string(), false),
                ("error".to_string(), true),
                (", then another ".to_string(), false),
                ("Error".to_string(), true),
            ]
        );
        assert!(highlight(&line, "timeout", HIT).is_none());
    }

    /// A match that starts in one colored span and ends in the next
    #[test]
    fn a_match_can_cross_spans() {
        let red = Style::default().fg(Color::Red);
        let line = Line::from(vec![
            Span::raw("12:00 "),
            Span::styled("conn", red),
            Span::raw("ection lost"),
        ]);
        let shown = highlight(&line, "connection", HIT).unwrap();
        assert_eq!(
            pieces(&shown),
            [
                ("12:00 ".to_string(), false),
                ("conn".to_string(), true),
                ("ection".to_string(), true),
                (" lost".to_string(), false),
            ]
        );
        // The span keeps its own color under the highlight
        assert_eq!(shown.spans[1].style.fg, Some(Color::Red));
    }

    #[test]
    fn non_ascii_text_around_a_match_is_left_intact() {
        let line = Line::from("日本語 error ✓");
        let shown = highlight(&line, "error", HIT).unwrap();
        let text: String = shown.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "日本語 error ✓");
    }
}
