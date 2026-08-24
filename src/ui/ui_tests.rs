#[cfg(test)]
mod tests {
    use crate::core::app_state::AppState;
    use crate::core::types::{
        AppEvent, Column, ColumnConfig, Container, ContainerKey, ContainerState, ContainerStats,
        SortState, ViewState,
    };
    use crate::ui::render::{UiStyles, render_ui};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use std::collections::HashMap;
    use tokio::sync::mpsc;

    /// Helper function to convert Buffer to a string representation
    fn buffer_to_string(buffer: &Buffer) -> String {
        let mut output = String::new();
        let area = buffer.area();

        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &buffer[(x, y)];
                output.push_str(cell.symbol());
            }
            if y < area.height - 1 {
                output.push('\n');
            }
        }

        output
    }

    /// Helper macro to assert snapshots with version redaction
    macro_rules! assert_snapshot_with_redaction {
        ($value:expr) => {{
            let mut settings = insta::Settings::clone_current();
            settings.add_filter(r"v\d+\.\d+\.\d+", "vX.X.X");
            settings.bind(|| {
                insta::assert_snapshot!($value);
            });
        }};
    }

    /// Helper function to create a mock AppState for testing
    fn create_test_app_state() -> AppState {
        let (tx, _rx) = mpsc::channel(100);
        AppState::new(
            HashMap::new(),
            tx,
            false,
            Column::Uptime,
            None, // sort_direction
            ColumnConfig::default(),
            None,
        )
    }

    /// Helper function to create a test container
    fn create_test_container(
        id: &str,
        name: &str,
        host_id: &str,
        cpu: f64,
        memory: f64,
        net_tx: f64,
        net_rx: f64,
    ) -> Container {
        create_test_container_full(id, name, host_id, cpu, memory, net_tx, net_rx, 0.0, 0.0)
    }

    /// Helper function to create a test container with disk I/O values
    #[allow(clippy::too_many_arguments)]
    fn create_test_container_full(
        id: &str,
        name: &str,
        host_id: &str,
        cpu: f64,
        memory: f64,
        net_tx: f64,
        net_rx: f64,
        disk_read: f64,
        disk_write: f64,
    ) -> Container {
        use chrono::Utc;

        // Create a test timestamp (e.g., 2 hours ago)
        let created = Some(Utc::now() - chrono::Duration::hours(2));

        Container {
            id: id.to_string(),
            name: name.to_string(),
            state: ContainerState::Running,
            health: None,
            created,
            stats: ContainerStats {
                cpu,
                memory,
                memory_used_bytes: (memory * 10_000_000.0) as u64, // Approximate based on percentage
                memory_limit_bytes: 1_000_000_000,                 // 1GB limit
                network_tx_bytes_per_sec: net_tx,
                network_rx_bytes_per_sec: net_rx,
                disk_read_bytes_per_sec: disk_read,
                disk_write_bytes_per_sec: disk_write,
                pids_current: 0,
                pids_limit: 0,
            },
            host_id: host_id.to_string(),
            dozzle_url: None,
            restart_count: None,
            compose_project: None,
        }
    }

    /// Finds the top-left cell of `text` in the buffer, scanning row by row.
    fn find_text_position(buffer: &Buffer, text: &str) -> Option<(u16, u16)> {
        let area = buffer.area();
        for y in 0..area.height {
            let mut row = String::new();
            for x in 0..area.width {
                row.push_str(buffer[(x, y)].symbol());
            }
            if let Some(byte_index) = row.find(text) {
                let column = row[..byte_index].chars().count() as u16;
                return Some((column, y));
            }
        }
        None
    }

    /// Renders the container list and returns the buffer.
    fn render_list(state: &mut AppState, styles: &UiStyles, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_ui(f, state, styles);
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn dozzle_container(id: &str, name: &str, dozzle: &str) -> Container {
        let mut container = create_test_container(id, name, "local", 10.0, 20.0, 0.0, 0.0);
        container.dozzle_url = Some(dozzle.to_string());
        container
    }

    /// The OSC 8 hyperlink must land on exactly the cell where the Name column
    /// starts. Rather than hardcoding an x-offset (which would silently rot if
    /// the column set or widths change), this locates the name in a plain render
    /// and asserts the linked render puts the escape at the same position.
    #[test]
    fn test_dozzle_hyperlink_lands_on_name_column() {
        let mut state = create_test_app_state();
        state.handle_event(AppEvent::ContainerCreated(dozzle_container(
            "abc123456789",
            "nginx",
            "https://l.dozzle.dev/",
        )));

        let plain = render_list(&mut state, &UiStyles::default(), 100, 20);
        let (x, y) = find_text_position(&plain, "nginx").expect("name should render");

        let linked = render_list(
            &mut state,
            &UiStyles::default().with_hyperlinks(true),
            100,
            20,
        );
        // The opening sequence rides on the first grapheme...
        assert_eq!(
            linked[(x, y)].symbol(),
            "\u{1b}]8;;https://l.dozzle.dev/container/abc123456789\u{1b}\\n",
            "expected the OSC 8 opener at ({x}, {y})"
        );
        // ...the terminator on the last...
        assert_eq!(linked[(x + 4, y)].symbol(), "x\u{1b}]8;;\u{1b}\\");
        // ...and the middle of the label stays ordinary text, so the row still
        // snapshots and copy-pastes as "nginx".
        assert_eq!(linked[(x + 1, y)].symbol(), "g");
        assert_eq!(linked[(x + 2, y)].symbol(), "i");
        assert_eq!(linked[(x + 3, y)].symbol(), "n");
    }

    /// Pins the overlay's replayed column layout against the width `Table`
    /// actually used for the `Constraint::Min(8)` Name column.
    ///
    /// The other tests can't catch a mismatch here: `test_dozzle_hyperlink_lands_on_name_column`
    /// uses a 5-char name that fits under the `Min` floor either way, and the
    /// swallowing test only counts differing cells, which stays true even if the
    /// closing escape lands mid-name. So this one uses a long name on a wide
    /// terminal and asserts the terminator sits on its *last* character — which
    /// only holds if `name_rect.width` matches the table's real column width.
    #[test]
    fn test_link_spans_full_name_at_wide_terminal() {
        let name = "compassionate_fermi"; // 19 chars, well over Min(8)
        let mut state = create_test_app_state();
        state.handle_event(AppEvent::ContainerCreated(dozzle_container(
            "e855f52b69e3",
            name,
            "http://localhost:8080",
        )));

        let width = 162;
        let plain = render_list(&mut state, &UiStyles::default(), width, 20);
        let (x, y) = find_text_position(&plain, name).expect("name should render in full");
        // The table itself must be rendering the whole name, or the assertion
        // below would be vacuous.
        assert_eq!(plain[(x + 18, y)].symbol(), "i");

        let linked = render_list(
            &mut state,
            &UiStyles::default().with_hyperlinks(true),
            width,
            20,
        );

        assert_eq!(
            linked[(x, y)].symbol(),
            "\u{1b}]8;;http://localhost:8080/container/e855f52b69e3\u{1b}\\c",
            "opener should sit on the first character of the name"
        );
        assert_eq!(
            linked[(x + 18, y)].symbol(),
            "i\u{1b}]8;;\u{1b}\\",
            "terminator should sit on the LAST character of the name, meaning the \
             overlay's replayed Name column width matches the table's"
        );
        // Everything between stays plain text, so the link covers the whole name.
        for offset in 1..18u16 {
            let symbol = linked[(x + offset, y)].symbol();
            assert!(
                !symbol.contains('\u{1b}'),
                "cell {offset} inside the name should be plain, got {symbol:?}"
            );
            assert_eq!(symbol, plain[(x + offset, y)].symbol());
        }
    }

    #[test]
    fn test_no_hyperlink_when_disabled_or_dozzle_unset() {
        let mut state = create_test_app_state();
        state.handle_event(AppEvent::ContainerCreated(dozzle_container(
            "abc123456789",
            "nginx",
            "https://l.dozzle.dev/",
        )));

        // Disabled by config/terminal detection.
        let disabled = render_list(&mut state, &UiStyles::default(), 100, 20);
        assert!(!buffer_to_string(&disabled).contains("\u{1b}]8;;"));

        // Enabled, but the host has no Dozzle URL configured.
        let mut plain_state = create_test_app_state();
        plain_state.handle_event(AppEvent::ContainerCreated(create_test_container(
            "abc123456789",
            "nginx",
            "local",
            10.0,
            20.0,
            0.0,
            0.0,
        )));
        let no_url = render_list(
            &mut plain_state,
            &UiStyles::default().with_hyperlinks(true),
            100,
            20,
        );
        assert!(!buffer_to_string(&no_url).contains("\u{1b}]8;;"));
    }

    /// Rows scroll under the header, so the overlay has to offset by
    /// `TableState::offset()` or links drift onto the wrong containers.
    #[test]
    fn test_hyperlink_follows_scroll_offset() {
        let mut state = create_test_app_state();
        for i in 0..40 {
            let container = dozzle_container(
                &format!("container{i:04}0000"),
                &format!("svc-{i:02}"),
                "https://dozzle.example.com",
            );
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
            state.sorted_container_keys.push(key);
        }
        // Select a row well past the first screenful so the table scrolls.
        state.table_state.select(Some(35));

        let styles = UiStyles::default().with_hyperlinks(true);
        let height = 20;
        // Render twice: the first pass is what updates TableState::offset.
        let _ = render_list(&mut state, &styles, 100, height);
        let buffer = render_list(&mut state, &styles, 100, height);

        let selected = state.table_state.selected().expect("a row is selected");
        let key = state.sorted_container_keys[selected].clone();
        let name = state.containers[&key].name.clone();

        let plain = render_list(&mut state, &UiStyles::default(), 100, height);
        let (x, y) = find_text_position(&plain, &name).expect("selected name is on screen");

        let symbol = buffer[(x, y)].symbol();
        let first_char = name.chars().next().unwrap();
        assert_eq!(
            symbol,
            format!(
                "\u{1b}]8;;https://dozzle.example.com/container/{}\u{1b}\\{first_char}",
                key.container_id
            ),
            "link for {name} should sit at ({x}, {y})"
        );
    }

    /// Regression test for the whole reason this needs care.
    ///
    /// `Buffer::diff` sizes a cell by its symbol's *display width*, and an OSC 8
    /// escape sequence measures dozens of columns wide while printing nothing.
    /// If the link cell doesn't declare `CellDiffOption::ForcedWidth`, the diff
    /// advances past that bogus width and silently drops every following cell on
    /// the row — the CPU and Memory columns just stop rendering.
    ///
    /// Runs at width 162 because progress bars only switch on at >= 128, which
    /// is what makes the swallowed columns obvious.
    #[test]
    fn test_hyperlink_does_not_swallow_following_columns() {
        let mut state = create_test_app_state();
        for (id, name) in [
            ("3616a6720244", "peaceful_jepsen"),
            ("312244cff86c", "wizardly_solomon"),
            ("e855f52b69e3", "compassionate_fermi"),
        ] {
            state.handle_event(AppEvent::ContainerCreated(dozzle_container(
                id,
                name,
                "http://localhost:8080",
            )));
        }

        let width = 162;
        let plain = render_list(&mut state, &UiStyles::default(), width, 20);
        let linked = render_list(
            &mut state,
            &UiStyles::default().with_hyperlinks(true),
            width,
            20,
        );

        for row in 4..7u16 {
            let differing: Vec<u16> = (0..width)
                .filter(|&cx| plain[(cx, row)].symbol() != linked[(cx, row)].symbol())
                .collect();

            // Only the first and last cell of the name carry escape sequences;
            // every other cell on the row, including the CPU and Memory bars,
            // must be byte-identical to the un-linked render.
            assert_eq!(
                differing.len(),
                2,
                "row {row}: expected only the link's first/last cells to differ, got {differing:?}"
            );
            for cx in differing {
                assert!(
                    linked[(cx, row)].symbol().contains('\u{1b}'),
                    "row {row}: cell {cx} differs but carries no escape sequence"
                );
            }

            // Spot-check that the bars actually survived rather than both
            // renders being blank.
            let rendered: String = (0..width)
                .map(|cx| plain[(cx, row)].symbol().to_string())
                .collect();
            assert!(
                rendered.contains("██░░"),
                "row {row}: progress bars missing from the baseline render"
            );
        }
    }

    /// End-to-end through the real backend, not just the buffer.
    ///
    /// `TestBackend` and the `CrosstermBackend` both consume `Buffer::diff`, but
    /// only the real one turns it into bytes. This asserts the escape sequence
    /// actually reaches the terminal *and* that the columns after it still do —
    /// the failure this feature originally shipped with was visible only here
    /// and in a real terminal, never in the pre-diff buffer.
    #[test]
    fn test_hyperlink_reaches_the_real_backend_without_eating_the_row() {
        use ratatui::backend::CrosstermBackend;
        use std::io::Write;
        use std::sync::{Arc, Mutex};

        #[derive(Clone)]
        struct Tap(Arc<Mutex<Vec<u8>>>);
        impl Write for Tap {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut state = create_test_app_state();
        state.handle_event(AppEvent::ContainerCreated(dozzle_container(
            "e855f52b69e3",
            "compassionate_fermi",
            "http://localhost:8080",
        )));

        let sink = Arc::new(Mutex::new(Vec::new()));
        let backend = CrosstermBackend::new(Tap(sink.clone()));
        let mut terminal = Terminal::with_options(
            backend,
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Fixed(ratatui::layout::Rect::new(0, 0, 162, 20)),
            },
        )
        .unwrap();

        let styles = UiStyles::default().with_hyperlinks(true);
        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let emitted = String::from_utf8_lossy(&sink.lock().unwrap()).to_string();

        assert!(
            emitted.contains("\u{1b}]8;;http://localhost:8080/container/e855f52b69e3\u{1b}\\"),
            "the OSC 8 sequence never reached the terminal"
        );
        assert!(
            emitted.contains("compassionate_fermi"),
            "the label should still be emitted as ordinary text"
        );
        // The CPU/Memory progress bars live to the right of the link. If the
        // diff swallowed the row, these never get written.
        assert!(
            emitted.contains("██░░"),
            "progress bars after the link were swallowed by the diff"
        );
        assert!(
            emitted.contains("10.0%"),
            "the CPU percentage after the link was swallowed by the diff"
        );
    }

    #[test]
    fn test_empty_container_list() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_single_host_container_list() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add containers from a single host
        let containers = vec![
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0),
            create_test_container(
                "def987654321",
                "postgres",
                "local",
                65.8,
                78.3,
                5120.0,
                10240.0,
            ),
            create_test_container("ghi111222333", "redis", "local", 15.2, 30.5, 512.0, 1024.0),
        ];

        for container in containers {
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
            state.sorted_container_keys.push(key);
        }

        // Select the first container
        state.table_state.select(Some(0));

        let backend = TestBackend::new(120, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_multi_host_container_list() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add containers from multiple hosts
        let containers = vec![
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0),
            create_test_container(
                "def987654321",
                "postgres",
                "user@server1",
                65.8,
                78.3,
                5120.0,
                10240.0,
            ),
            create_test_container(
                "ghi111222333",
                "redis",
                "192.168.1.100:2375",
                15.2,
                30.5,
                512.0,
                1024.0,
            ),
        ];

        for container in containers {
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
            state.sorted_container_keys.push(key);
        }

        // Select the second container
        state.table_state.select(Some(1));

        // Use wider terminal (150) to accommodate Host column without truncation
        let backend = TestBackend::new(150, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_high_resource_usage() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add containers with varying resource usage to test color coding
        let containers = vec![
            create_test_container(
                "low12345678",
                "low-usage",
                "local",
                15.0,
                20.0,
                100.0,
                200.0,
            ),
            create_test_container(
                "med12345678",
                "medium-usage",
                "local",
                55.0,
                65.0,
                1024000.0,
                2048000.0,
            ),
            create_test_container(
                "high12345678",
                "high-usage",
                "local",
                95.0,
                99.0,
                104857600.0,
                209715200.0,
            ),
        ];

        for container in containers {
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
            state.sorted_container_keys.push(key);
        }

        let backend = TestBackend::new(120, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_log_view_empty() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add a container
        let container =
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0);
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        state.containers.insert(key.clone(), container);

        // Switch to log view
        state.view_state = ViewState::LogView(key.clone());
        state.is_at_bottom = true;

        // Create empty log state
        use crate::core::types::LogState;
        let log_state = LogState::new(key.clone(), None);
        state.log_state = Some(log_state);

        let backend = TestBackend::new(120, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_log_view_with_content() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add a container
        let container =
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0);
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        state.containers.insert(key.clone(), container);

        // Switch to log view and add some log lines
        state.view_state = ViewState::LogView(key.clone());
        state.is_at_bottom = true;

        // Create log entries instead of formatted text
        use crate::core::types::LogState;
        use crate::docker::logs::LogEntry;
        use chrono::{Local, TimeZone, Utc};

        // Create timestamps in local timezone, then convert to UTC for consistent display
        // This ensures tests work regardless of the machine's timezone
        let base_time = Local.with_ymd_and_hms(2025, 10, 29, 10, 15, 30).unwrap();
        let base_utc = base_time.with_timezone(&Utc);

        let log_entries = vec![
            LogEntry::parse(&format!(
                "{}Z Starting server on port 8080",
                base_utc.format("%Y-%m-%dT%H:%M:%S")
            ))
            .unwrap(),
            LogEntry::parse(&format!(
                "{}Z Database connection established",
                (base_utc + chrono::Duration::seconds(1)).format("%Y-%m-%dT%H:%M:%S")
            ))
            .unwrap(),
            LogEntry::parse(&format!(
                "{}Z Listening for requests...",
                (base_utc + chrono::Duration::seconds(2)).format("%Y-%m-%dT%H:%M:%S")
            ))
            .unwrap(),
            LogEntry::parse(&format!(
                "{}Z GET /api/users 200 OK",
                (base_utc + chrono::Duration::seconds(3)).format("%Y-%m-%dT%H:%M:%S")
            ))
            .unwrap(),
        ];

        let mut log_state = LogState::new(key.clone(), None);
        log_state.set_entries(log_entries);
        state.log_state = Some(log_state);

        let backend = TestBackend::new(120, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_log_view_manual_scroll() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add a container
        let container =
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0);
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        state.containers.insert(key.clone(), container);

        // Switch to log view with manual scroll
        state.view_state = ViewState::LogView(key.clone());
        state.is_at_bottom = false; // Manual scroll mode

        // Create log state with log content
        use crate::core::types::LogState;
        use crate::docker::logs::LogEntry;
        use chrono::{Local, TimeZone, Utc};

        // Create timestamps in local timezone, then convert to UTC for consistent display
        let base_time = Local.with_ymd_and_hms(2025, 10, 29, 10, 15, 30).unwrap();
        let base_utc = base_time.with_timezone(&Utc);

        let log_entries = vec![
            LogEntry::parse(&format!(
                "{}Z Log line 1",
                base_utc.format("%Y-%m-%dT%H:%M:%S")
            ))
            .unwrap(),
            LogEntry::parse(&format!(
                "{}Z Log line 2",
                (base_utc + chrono::Duration::seconds(1)).format("%Y-%m-%dT%H:%M:%S")
            ))
            .unwrap(),
            LogEntry::parse(&format!(
                "{}Z Log line 3",
                (base_utc + chrono::Duration::seconds(2)).format("%Y-%m-%dT%H:%M:%S")
            ))
            .unwrap(),
        ];

        let mut log_state = LogState::new(key.clone(), None);
        log_state.set_entries(log_entries);
        log_state.scroll_offset = 5;
        state.log_state = Some(log_state);

        let backend = TestBackend::new(120, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_container_list_with_stopped_containers() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        use chrono::Utc;

        // Add running containers
        let running_containers = vec![
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0),
            create_test_container(
                "def987654321",
                "postgres",
                "local",
                65.8,
                78.3,
                5120.0,
                10240.0,
            ),
        ];

        for container in running_containers {
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
            state.sorted_container_keys.push(key);
        }

        // Add stopped containers
        let stopped_containers = vec![
            Container {
                id: "stop12345678".to_string(),
                name: "old-redis".to_string(),
                state: ContainerState::Exited,
                health: None,
                created: Some(Utc::now() - chrono::Duration::days(1)),
                stats: ContainerStats::default(), // Stats should not be shown
                host_id: "local".to_string(),
                dozzle_url: None,
                restart_count: None,
                compose_project: None,
            },
            Container {
                id: "dead12345678".to_string(),
                name: "failed-app".to_string(),
                state: ContainerState::Dead,
                health: None,
                created: Some(Utc::now() - chrono::Duration::hours(3)),
                stats: ContainerStats::default(), // Stats should not be shown
                host_id: "local".to_string(),
                dozzle_url: None,
                restart_count: None,
                compose_project: None,
            },
        ];

        for container in stopped_containers {
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
            state.sorted_container_keys.push(key);
        }

        // Select the first container
        state.table_state.select(Some(0));

        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_wide_terminal_with_progress_bars() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add a container with stats
        let container =
            create_test_container("abc123456789", "nginx", "local", 45.5, 62.3, 1024.0, 2048.0);

        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        state.containers.insert(key.clone(), container);
        state.sorted_container_keys.push(key);

        // Use a wide terminal (>= 128 chars) to trigger progress bar display
        let backend = TestBackend::new(150, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);

        // Verify that progress bars are present (containing █ or ░ characters)
        assert!(
            output.contains('█') || output.contains('░'),
            "Wide terminal (150 chars) should display progress bars"
        );

        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_search_mode_active() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add some containers
        let containers = vec![
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0),
            create_test_container(
                "def987654321",
                "postgres",
                "local",
                65.8,
                78.3,
                5120.0,
                10240.0,
            ),
            create_test_container("ghi111222333", "redis", "local", 15.2, 30.5, 512.0, 1024.0),
        ];

        for container in containers {
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
            state.sorted_container_keys.push(key);
        }

        // Enter search mode with some input
        state.view_state = ViewState::SearchMode;
        state.search_input = tui_input::Input::new("ngi".to_string());

        let backend = TestBackend::new(120, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);

        // Verify search bar is visible with "/" prefix
        assert!(output.contains("/ngi"), "Search mode should show /ngi");

        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_filtering_active_search_mode_off() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add some containers
        let containers = vec![
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0),
            create_test_container(
                "def987654321",
                "postgres",
                "local",
                65.8,
                78.3,
                5120.0,
                10240.0,
            ),
        ];

        for container in containers {
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
            state.sorted_container_keys.push(key);
        }

        // Set up filter but not in search mode (user exited search mode with filter active)
        state.view_state = ViewState::ContainerList;
        state.search_input = tui_input::Input::new("nginx".to_string());

        let backend = TestBackend::new(120, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);

        // Verify search bar shows "Filtering:" prefix instead of "/"
        assert!(
            output.contains("Filtering: nginx"),
            "Should show 'Filtering: nginx'"
        );

        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_help_popup_enabled() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add a container
        let container =
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0);
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        state.containers.insert(key.clone(), container);
        state.sorted_container_keys.push(key);

        // Enable help popup
        state.show_help = true;

        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);

        // Verify help content is visible
        assert!(output.contains("Help"), "Should show help popup");
        assert!(
            output.contains("Navigation") || output.contains("Sorting"),
            "Should show help content sections"
        );

        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_action_menu_enabled() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add a running container
        let container =
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0);
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        state.containers.insert(key.clone(), container);
        state.sorted_container_keys.push(key.clone());
        state.table_state.select(Some(0));

        // Show action menu
        state.view_state = ViewState::ActionMenu(key);
        state.action_menu_state.select(Some(0));

        let backend = TestBackend::new(120, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);

        // Verify action menu is visible with actions
        assert!(output.contains("Actions"), "Should show action menu");
        assert!(
            output.contains("Stop") || output.contains("Restart"),
            "Should show container actions"
        );

        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_connection_error_notification() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        // Add a successful container
        let container =
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0);
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        state.containers.insert(key.clone(), container);
        state.sorted_container_keys.push(key);

        // Add a connection error for a remote host
        use std::time::Instant;
        state.connection_errors.insert(
            "user@server1".to_string(),
            (
                "Failed to connect: Connection refused".to_string(),
                Instant::now(),
            ),
        );

        let backend = TestBackend::new(140, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);

        // Verify error notification is visible in top right
        assert!(output.contains("user@server1"), "Should show failed host");
        assert!(
            output.contains("Failed to connect") || output.contains("Connection refused"),
            "Should show error message"
        );

        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_column_selector_popup() {
        let mut state = create_test_app_state();

        let container =
            create_test_container("abc123def456", "nginx", "local", 25.0, 50.0, 1024.0, 2048.0);
        let key = ContainerKey::new("local".to_string(), "abc123def456".to_string());
        state.containers.insert(key, container);
        state.sort_containers();
        state.table_state.select(Some(0));

        state.view_state = ViewState::ColumnSelector;
        state.column_selector_state.select(Some(0));

        let styles = UiStyles::default();
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_container_list_with_hidden_columns() {
        let mut state = create_test_app_state();

        let id_idx = state
            .column_config
            .columns
            .iter()
            .position(|(c, _)| *c == Column::Id)
            .unwrap();
        state.column_config.toggle(id_idx);
        let net_tx_idx = state
            .column_config
            .columns
            .iter()
            .position(|(c, _)| *c == Column::NetTx)
            .unwrap();
        state.column_config.toggle(net_tx_idx);

        let container =
            create_test_container("abc123def456", "nginx", "local", 25.0, 50.0, 1024.0, 2048.0);
        let key = ContainerKey::new("local".to_string(), "abc123def456".to_string());
        state.containers.insert(key, container);
        state.sort_containers();
        state.table_state.select(Some(0));

        let styles = UiStyles::default();
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert_snapshot_with_redaction!(output);
    }

    /// Populates the given state with `count` running containers on the local host.
    fn populate_containers(state: &mut AppState, count: usize) {
        for i in 0..count {
            let container = create_test_container(
                &format!("id{i:010}"),
                &format!("c{i}"),
                "local",
                1.0,
                1.0,
                0.0,
                0.0,
            );
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
            state.sorted_container_keys.push(key);
        }
        state.table_state.select(Some(0));
    }

    #[test]
    fn test_page_down_moves_by_viewport_height() {
        let mut state = create_test_app_state();
        populate_containers(&mut state, 100);
        state.last_list_viewport_height = 10;

        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::PageDown,
            KeyModifiers::NONE,
        )));

        assert_eq!(state.table_state.selected(), Some(10));
    }

    #[test]
    fn test_page_up_moves_by_viewport_height() {
        let mut state = create_test_app_state();
        populate_containers(&mut state, 100);
        state.last_list_viewport_height = 10;
        state.table_state.select(Some(25));

        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::PageUp,
            KeyModifiers::NONE,
        )));

        assert_eq!(state.table_state.selected(), Some(15));
    }

    #[test]
    fn test_page_down_clamps_to_last_container() {
        let mut state = create_test_app_state();
        populate_containers(&mut state, 5);
        state.last_list_viewport_height = 10;

        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::PageDown,
            KeyModifiers::NONE,
        )));

        assert_eq!(state.table_state.selected(), Some(4));
    }

    #[test]
    fn test_page_up_clamps_to_first_container() {
        let mut state = create_test_app_state();
        populate_containers(&mut state, 100);
        state.last_list_viewport_height = 10;
        state.table_state.select(Some(3));

        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::PageUp,
            KeyModifiers::NONE,
        )));

        assert_eq!(state.table_state.selected(), Some(0));
    }

    #[test]
    fn test_home_and_end_jump_to_bounds() {
        let mut state = create_test_app_state();
        populate_containers(&mut state, 50);
        state.table_state.select(Some(20));

        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::End,
            KeyModifiers::NONE,
        )));
        assert_eq!(state.table_state.selected(), Some(49));

        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::Home,
            KeyModifiers::NONE,
        )));
        assert_eq!(state.table_state.selected(), Some(0));
    }

    /// A reconnect re-sends `InitialContainerList` as the authoritative list for
    /// the host, so containers that disappeared while the daemon was down must be
    /// dropped and other hosts must be left alone.
    #[test]
    fn test_initial_container_list_resyncs_host() {
        let mut state = create_test_app_state();
        state.show_all_containers = true;

        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![
                create_test_container("aaaaaaaaaaaa", "nginx", "local", 1.0, 1.0, 0.0, 0.0),
                create_test_container("bbbbbbbbbbbb", "redis", "local", 1.0, 1.0, 0.0, 0.0),
            ],
        ));
        state.handle_event(AppEvent::InitialContainerList(
            "remote".to_string(),
            vec![create_test_container(
                "cccccccccccc",
                "postgres",
                "remote",
                1.0,
                1.0,
                0.0,
                0.0,
            )],
        ));
        assert_eq!(state.containers.len(), 3);

        // "local" reconnects: redis is gone, a new container appeared.
        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![
                create_test_container("aaaaaaaaaaaa", "nginx", "local", 1.0, 1.0, 0.0, 0.0),
                create_test_container("dddddddddddd", "caddy", "local", 1.0, 1.0, 0.0, 0.0),
            ],
        ));

        let mut names: Vec<&str> = state.containers.values().map(|c| c.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["caddy", "nginx", "postgres"]);

        // No duplicated keys in the rendered list
        assert_eq!(state.sorted_container_keys.len(), 3);
    }

    /// A resync zeroes the host's stats, so under a stats-based sort every row
    /// compares equal and the list can come back in a different order. The cursor
    /// must follow the selected *container*, not its old row index — otherwise the
    /// next Enter opens the action menu on something the user never picked.
    #[test]
    fn test_resync_keeps_selection_on_same_container() {
        let mut state = create_test_app_state();
        state.sort_state = SortState::new(Column::Cpu);

        let before = vec![
            create_test_container("aaaaaaaaaaaa", "alpha", "local", 90.0, 1.0, 0.0, 0.0),
            create_test_container("bbbbbbbbbbbb", "beta", "local", 50.0, 1.0, 0.0, 0.0),
            create_test_container("cccccccccccc", "gamma", "local", 10.0, 1.0, 0.0, 0.0),
        ];
        state.handle_event(AppEvent::InitialContainerList("local".to_string(), before));

        // Select "gamma" — last row while sorted by CPU descending.
        let gamma = ContainerKey::new("local".to_string(), "cccccccccccc".to_string());
        let gamma_row = state
            .sorted_container_keys
            .iter()
            .position(|k| *k == gamma)
            .unwrap();
        state.table_state.select(Some(gamma_row));

        // Reconnect: same containers, but stats reset to zero as they would be.
        let after = vec![
            create_test_container("aaaaaaaaaaaa", "alpha", "local", 0.0, 0.0, 0.0, 0.0),
            create_test_container("bbbbbbbbbbbb", "beta", "local", 0.0, 0.0, 0.0, 0.0),
            create_test_container("cccccccccccc", "gamma", "local", 0.0, 0.0, 0.0, 0.0),
        ];
        state.handle_event(AppEvent::InitialContainerList("local".to_string(), after));

        let selected = state
            .table_state
            .selected()
            .and_then(|i| state.sorted_container_keys.get(i))
            .expect("a row should still be selected");
        assert_eq!(
            *selected, gamma,
            "selection should follow the container across a resync"
        );
    }

    /// If the selected container is gone after a reconnect, the selection falls
    /// back to a valid row rather than pointing past the end of the list.
    #[test]
    fn test_resync_falls_back_when_selected_container_disappears() {
        let mut state = create_test_app_state();

        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![
                create_test_container("aaaaaaaaaaaa", "alpha", "local", 1.0, 1.0, 0.0, 0.0),
                create_test_container("bbbbbbbbbbbb", "beta", "local", 1.0, 1.0, 0.0, 0.0),
            ],
        ));
        state.table_state.select(Some(1));

        // Only one container survives the restart.
        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![create_test_container(
                "aaaaaaaaaaaa",
                "alpha",
                "local",
                1.0,
                1.0,
                0.0,
                0.0,
            )],
        ));

        assert_eq!(state.sorted_container_keys.len(), 1);
        assert_eq!(state.table_state.selected(), Some(0));
    }

    /// An empty list from a reconnected host clears its stale containers without
    /// leaving a dangling selection.
    #[test]
    fn test_initial_container_list_empty_clears_host() {
        let mut state = create_test_app_state();

        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![create_test_container(
                "aaaaaaaaaaaa",
                "nginx",
                "local",
                1.0,
                1.0,
                0.0,
                0.0,
            )],
        ));
        assert_eq!(state.table_state.selected(), Some(0));

        state.handle_event(AppEvent::InitialContainerList("local".to_string(), vec![]));

        assert!(state.containers.is_empty());
        assert!(state.sorted_container_keys.is_empty());
        assert_eq!(state.table_state.selected(), None);
    }

    #[test]
    fn test_host_disconnect_and_reconnect_events() {
        let mut state = create_test_app_state();

        state.handle_event(AppEvent::HostDisconnected("local".to_string()));
        assert!(state.disconnected_hosts.contains("local"));

        state.handle_event(AppEvent::HostReconnected("local".to_string()));
        assert!(state.disconnected_hosts.is_empty());
    }

    #[test]
    fn test_reconnecting_banner() {
        let mut state = create_test_app_state();
        let styles = UiStyles::default();

        let container =
            create_test_container("abc123456789", "nginx", "local", 25.5, 45.2, 1024.0, 2048.0);
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        state.containers.insert(key.clone(), container);
        state.sorted_container_keys.push(key);

        state.handle_event(AppEvent::HostDisconnected("user@server1".to_string()));

        let backend = TestBackend::new(140, 25);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let output = buffer_to_string(&terminal.backend().buffer().clone());

        assert!(output.contains("user@server1"), "Should show the lost host");
        assert!(
            output.contains("reconnecting"),
            "Should say it is reconnecting"
        );

        assert_snapshot_with_redaction!(output);
    }
}
