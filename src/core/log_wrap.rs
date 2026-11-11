//! Word wrapping for the log view.
//!
//! Ratatui's `Wrap` restarts every wrapped row at column 0, under the
//! timestamp, which hides where one entry ends and the next begins. This wraps
//! the same way (at spaces, mid-word only when a word is longer than the row)
//! but starts continuation rows `indent` columns in. Doing the wrapping here
//! also makes the row counts the scroll math uses exact.

use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

const SPACES: &str = "                                ";

/// Calls `on_break` with the byte offset (into the line's concatenated spans)
/// where each wrapped row after the first starts.
fn for_each_break(line: &Line, width: usize, indent: usize, mut on_break: impl FnMut(usize)) {
    if width == 0 {
        return;
    }
    let continuation = width.saturating_sub(indent).max(1);
    let mut capacity = width;
    let mut row_width = 0;
    // Offset just past the last space on this row, and the row's width there
    let mut last_space: Option<(usize, usize)> = None;
    let mut offset = 0;

    for c in line.spans.iter().flat_map(|span| span.content.chars()) {
        let char_width = c.width().unwrap_or(0);
        if row_width + char_width > capacity && row_width > 0 {
            match last_space {
                // The word being carried down has to fit on the next row
                Some((at, width_there)) if row_width - width_there + char_width <= continuation => {
                    on_break(at);
                    row_width -= width_there;
                }
                _ => {
                    on_break(offset);
                    row_width = 0;
                }
            }
            capacity = continuation;
            last_space = None;
        }
        row_width += char_width;
        offset += c.len_utf8();
        if c == ' ' {
            last_space = Some((offset, row_width));
        }
    }
}

/// How many rows `line` takes when wrapped to `width`, with rows after the
/// first indented by `indent`.
pub fn wrapped_rows(line: &Line, width: usize, indent: usize) -> usize {
    let mut rows = 1;
    for_each_break(line, width, indent, |_| rows += 1);
    rows
}

/// Byte offsets (into the line's concatenated spans) where each wrapped row
/// after the first starts
pub fn wrap_breaks(line: &Line, width: usize, indent: usize) -> Vec<usize> {
    let mut breaks = Vec::new();
    for_each_break(line, width, indent, |at| breaks.push(at));
    breaks
}

/// Appends `line`, wrapped to `width`, to `rows`. Rows after the first start
/// with `indent` spaces.
pub fn wrap_into<'a>(rows: &mut Vec<Line<'a>>, line: &'a Line, width: usize, indent: usize) {
    let mut breaks = Vec::new();
    for_each_break(line, width, indent, |at| breaks.push(at));
    if breaks.is_empty() {
        rows.push(line.clone());
        return;
    }

    let indent = &SPACES[..indent.min(SPACES.len())];
    let mut breaks = breaks.into_iter().peekable();
    let mut row: Vec<Span<'a>> = Vec::new();
    let mut offset = 0;
    for span in &line.spans {
        let content: &'a str = &span.content;
        let mut start = 0;
        while let Some(&at) = breaks.peek()
            && at < offset + content.len()
        {
            let cut = at - offset;
            if cut > start {
                row.push(Span::styled(&content[start..cut], span.style));
            }
            rows.push(Line::from(std::mem::take(&mut row)));
            if !indent.is_empty() {
                row.push(Span::raw(indent));
            }
            start = cut;
            breaks.next();
        }
        if start < content.len() {
            row.push(Span::styled(&content[start..], span.style));
        }
        offset += content.len();
    }
    rows.push(Line::from(row));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Style};

    fn wrap(line: &Line, width: usize, indent: usize) -> Vec<String> {
        let mut rows = Vec::new();
        wrap_into(&mut rows, line, width, indent);
        assert_eq!(rows.len(), wrapped_rows(line, width, indent));
        for row in &rows {
            assert!(row.width() <= width, "{row:?} is wider than {width}");
        }
        rows.iter()
            .map(|row| row.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn short_lines_are_one_row() {
        assert_eq!(wrap(&Line::from("12:00 hello"), 20, 6), ["12:00 hello"]);
        assert_eq!(wrap(&Line::from(""), 20, 6), [""]);
    }

    #[test]
    fn continuation_rows_hang_under_the_message() {
        let line = Line::from("12:00 the quick brown fox jumps over the lazy dog");
        assert_eq!(
            wrap(&line, 20, 6),
            [
                "12:00 the quick ",
                "      brown fox ",
                "      jumps over ",
                "      the lazy dog"
            ]
        );
    }

    #[test]
    fn a_word_longer_than_the_row_is_cut() {
        let line = Line::from(format!("12:00 {}", "x".repeat(30)));
        // It starts on the first row rather than leaving that row empty
        assert_eq!(
            wrap(&line, 20, 6),
            [
                &format!("12:00 {}", "x".repeat(14)),
                &format!("      {}", "x".repeat(14)),
                "      xx"
            ]
        );
    }

    #[test]
    fn breaks_fall_inside_and_between_spans_keeping_styles() {
        let red = Style::default().fg(Color::Red);
        let line = Line::from(vec![
            Span::raw("12:00 "),
            Span::styled("alpha beta", red),
            Span::raw(" gamma delta"),
        ]);
        let mut rows = Vec::new();
        wrap_into(&mut rows, &line, 14, 6);
        let text: Vec<String> = rows
            .iter()
            .map(|row| row.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(
            text,
            ["12:00 alpha ", "      beta ", "      gamma ", "      delta"]
        );
        // "beta" is still red on its own row, after the unstyled indent
        assert_eq!(rows[1].spans[1].style, red);
        assert_eq!(rows[1].spans[0].style, Style::default());
    }

    #[test]
    fn wide_characters_never_overflow_a_row() {
        let line = Line::from(format!("12:00 {}", "日本語".repeat(8)));
        let rows = wrap(&line, 15, 6);
        assert!(rows.len() > 3);
    }

    #[test]
    fn no_indent_wraps_to_column_zero() {
        let line = Line::from("alpha beta gamma");
        assert_eq!(wrap(&line, 11, 0), ["alpha beta ", "gamma"]);
    }
}
