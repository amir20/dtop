use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::core::app_state::AppState;
use crate::core::log_search::highlight;
use crate::core::log_wrap::{wrap_breaks, wrap_into};
use crate::core::types::ContainerKey;

use super::log_links::{LogLink, link_pieces};
use super::render::UiStyles;

/// Search matches, and those on the line `n`/`N` last jumped to
const MATCH: Style = Style::new().fg(Color::Black).bg(Color::Yellow);
const CURRENT_MATCH: Style = Style::new().fg(Color::Black).bg(Color::LightMagenta);

/// Find the entry index and sub-line offset for a given visual line position.
/// Returns (entry_index, sub_line_offset) where sub_line_offset is the number
/// of visual lines into the entry that the scroll position falls.
fn find_visible_start(line_rows: &[usize], visual_line: usize) -> (usize, usize) {
    let mut accumulated = 0;
    for (i, &rows) in line_rows.iter().enumerate() {
        if accumulated + rows > visual_line {
            return (i, visual_line - accumulated);
        }
        accumulated += rows;
    }
    // Past the end — return last entry with no sub-offset
    (line_rows.len().saturating_sub(1), 0)
}

/// Keys worth knowing in the log view, most useful first
const KEY_HINTS: [&str; 4] = ["Esc back", "t timestamps", "/ search", "? help"];

/// The title row: what is shown, then as many key hints as fit in `width`
fn log_title(container: &str, host: &str, status: &str, width: usize) -> String {
    use unicode_width::UnicodeWidthStr;

    let mut title = format!("Logs: {container} ({host})");
    if !status.is_empty() {
        title.push(' ');
        title.push_str(status);
    }
    let mut used = title.width();
    for hint in KEY_HINTS {
        used += 3 + hint.width();
        if used > width {
            break;
        }
        title.push_str(" · ");
        title.push_str(hint);
    }
    title
}

/// Renders the log view for a specific container
pub fn render_log_view(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    container_key: &ContainerKey,
    state: &mut AppState,
    styles: &UiStyles,
) {
    let Some(log_state) = &mut state.log_state else {
        return;
    };

    if &log_state.container_key != container_key {
        return;
    }

    let container_name = state
        .containers
        .get(container_key)
        .map(|c| c.name.as_str())
        .unwrap_or("Unknown");

    // Calculate visible height (subtract 2 for top and bottom border)
    let visible_height = area.height.saturating_sub(2) as usize;

    // Inner width for wrap calculations (subtract 2 for left/right border)
    let inner_width = area.width.saturating_sub(2) as usize;

    // Store viewport dimensions for scroll calculations
    state.last_viewport_height = visible_height;
    state.last_viewport_width = inner_width;

    // Cached, and updated in place as lines stream in
    let total_rows = log_state.total_rows(inner_width);

    // Max scroll: enough so that the last visual line is at the bottom of the viewport
    let max_scroll = total_rows.saturating_sub(visible_height);

    // Determine actual scroll offset (in visual lines)
    let actual_scroll = if state.is_at_bottom {
        max_scroll
    } else {
        log_state.scroll_offset.min(max_scroll)
    };

    // Update is_at_bottom based on actual position
    state.is_at_bottom = actual_scroll >= max_scroll;

    // Update scroll offset to actual (for proper clamping)
    log_state.scroll_offset = actual_scroll;

    // Find the first visible entry and sub-line offset within it.
    // This also gives us the entry index for progress calculation.
    let (first_entry_idx, sub_line_offset) =
        find_visible_start(log_state.line_rows(), actual_scroll);

    // Determine status indicator
    let status_indicator = if log_state.fetching_older {
        "[Loading...]".to_string()
    } else if state.is_at_bottom {
        "[LIVE]".to_string()
    } else if let Some(progress) = log_state.calculate_progress(first_entry_idx) {
        if log_state.has_more_history || progress > 0.0 {
            format!("[{progress:.0}%]")
        } else {
            "[0%]".to_string()
        }
    } else {
        String::new()
    };

    // Wrap only the visible slice of lines — O(viewport) instead of O(n).
    // We need enough rows to fill visible_height + sub_line_offset (to account
    // for the partial first entry that gets scrolled past).
    let needed_rows = visible_height + sub_line_offset;
    let indent = log_state.wrap_indent(inner_width);
    // With a search, matching lines are redrawn with the matches highlighted.
    // At most one line per row can show, so this stays O(viewport) too.
    let needle = log_state.search.as_ref().map(|search| search.needle());
    let current = log_state.search.as_ref().and_then(|search| search.current);
    let highlighted: Vec<Option<Line<'static>>> = match needle.as_deref() {
        Some(needle) if !needle.is_empty() => log_state
            .formatted_lines
            .iter()
            .enumerate()
            .skip(first_entry_idx)
            .take(needed_rows)
            .map(|(index, line)| {
                let style = if current == Some(index) {
                    CURRENT_MATCH
                } else {
                    MATCH
                };
                highlight(line, needle, style)
            })
            .collect(),
        _ => Vec::new(),
    };

    let mut visible_rows: Vec<Line> = Vec::with_capacity(needed_rows);
    // Not under the help popup, which covers the text
    let link_urls = styles.hyperlinks && !state.show_help;
    for (i, line) in log_state
        .formatted_lines
        .iter()
        .skip(first_entry_idx)
        .enumerate()
    {
        let line = highlighted.get(i).and_then(Option::as_ref).unwrap_or(line);
        let first_row = visible_rows.len();
        wrap_into(&mut visible_rows, line, inner_width, indent);

        if link_urls && line.spans.iter().any(|s| s.content.contains("http")) {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let breaks = wrap_breaks(line, inner_width, indent);
            for piece in link_pieces(&text, &breaks, indent) {
                // Rows scrolled off the top, or past the bottom, aren't drawn.
                // The text starts on the row under the title.
                let row = (first_row + piece.row).checked_sub(sub_line_offset);
                if let Some(row) = row.filter(|&row| row < visible_height) {
                    state.log_links.push(LogLink {
                        x: area.x + piece.column as u16,
                        y: area.y + 1 + row as u16,
                        label: text[piece.text].to_string(),
                        url: text[piece.url].to_string(),
                    });
                }
            }
        }

        if visible_rows.len() >= needed_rows {
            break;
        }
    }
    visible_rows.drain(..sub_line_offset.min(visible_rows.len()));
    visible_rows.truncate(visible_height);

    // Already wrapped (`log_wrap`), so the paragraph draws rows as they are
    let log_widget = Paragraph::new(Text::from(visible_rows)).block(
        Block::default()
            .title(log_title(
                container_name,
                &container_key.host_id,
                &status_indicator,
                // The scrollbar's arrow takes the last column of the title row
                area.width.saturating_sub(1) as usize,
            ))
            .style(styles.border),
    );

    f.render_widget(log_widget, area);

    // Underlined here; the event loop makes them links (`LinkWriter`)
    for link in &state.log_links {
        let width = unicode_width::UnicodeWidthStr::width(link.label.as_str()) as u16;
        f.buffer_mut()
            .set_style(Rect::new(link.x, link.y, width, 1), styles.link);
    }

    // Render scrollbar on the right side
    let mut scrollbar_state = ScrollbarState::default()
        .content_length(total_rows)
        .viewport_content_length(visible_height)
        .position(actual_scroll + visible_height);

    let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);

    f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}

#[cfg(test)]
mod tests {
    use super::log_title;

    #[test]
    fn title_drops_hints_from_the_end_as_it_narrows() {
        assert_eq!(
            log_title("nginx", "local", "[LIVE]", 120),
            "Logs: nginx (local) [LIVE] · Esc back · t timestamps · / search · ? help"
        );
        assert_eq!(
            log_title("nginx", "local", "[LIVE]", 60),
            "Logs: nginx (local) [LIVE] · Esc back · t timestamps"
        );
        assert_eq!(
            log_title("nginx", "local", "[LIVE]", 30),
            "Logs: nginx (local) [LIVE]"
        );
        // The name and status are never cut to make room
        assert_eq!(log_title("nginx", "local", "", 5), "Logs: nginx (local)");
    }
}
