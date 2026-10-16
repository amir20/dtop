#[cfg(test)]
mod tests {
    use crate::core::app_state::AppState;
    use crate::core::types::{
        AppEvent, Column, ColumnConfig, Container, ContainerKey, ContainerState, ContainerStats,
        HealthStatus, HostInfo, SortState, SystemStats, ViewState,
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
            image: Some(format!("{name}:latest")),
            ports: None,
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

    /// State with a local machine sample and two Docker hosts, for the stats header.
    fn create_system_stats_state() -> AppState {
        let mut state = create_test_app_state();
        let mut stopped =
            create_test_container("ccc000000003", "old-job", "local", 0.0, 0.0, 0.0, 0.0);
        stopped.state = ContainerState::Exited;
        for (i, mut container) in [
            create_test_container(
                "abc123456789",
                "nginx",
                "local",
                150.0,
                45.2,
                1024.0,
                2048.0,
            ),
            create_test_container("bbb222222222", "redis", "local", 50.0, 30.0, 0.0, 0.0),
            stopped,
            create_test_container(
                "def987654321",
                "postgres",
                "user@server1",
                380.0,
                78.3,
                0.0,
                0.0,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            // Distinct creation times so the default uptime sort is deterministic
            container.created = Some(chrono::Utc::now() - chrono::Duration::hours(i as i64 + 1));
            state.handle_event(AppEvent::ContainerCreated(container));
        }
        state.handle_event(AppEvent::HostInfo(
            "local".to_string(),
            HostInfo {
                cpus: 8,
                memory_total_bytes: 8 * 1024 * 1024 * 1024,
            },
        ));
        state.handle_event(AppEvent::HostInfo(
            "user@server1".to_string(),
            HostInfo {
                cpus: 4,
                memory_total_bytes: 2 * 1024 * 1024 * 1024,
            },
        ));
        state.handle_event(AppEvent::SystemStats(SystemStats {
            hostname: "workstation".to_string(),
            cpu: 62.5,
            cpus: 10,
            memory_used_bytes: 12 * 1024 * 1024 * 1024,
            memory_total_bytes: 16 * 1024 * 1024 * 1024,
            swap_used_bytes: 512 * 1024 * 1024,
            swap_total_bytes: 2 * 1024 * 1024 * 1024,
            net_rx_bytes_per_sec: 3.5 * 1024.0 * 1024.0,
            net_tx_bytes_per_sec: 420.0 * 1024.0,
            load_average: [1.25, 0.9, 0.75],
            uptime_secs: 3 * 86_400 + 4 * 3_600,
        }));
        state.handle_event(AppEvent::RemoteSystemStats(
            "user@server1".to_string(),
            SystemStats {
                hostname: "user@server1".to_string(),
                cpu: 97.0,
                cpus: 4,
                memory_used_bytes: 1900 * 1024 * 1024,
                memory_total_bytes: 2 * 1024 * 1024 * 1024,
                swap_used_bytes: 0,
                swap_total_bytes: 0,
                net_rx_bytes_per_sec: 12.0 * 1024.0 * 1024.0,
                net_tx_bytes_per_sec: 8.0 * 1024.0 * 1024.0,
                load_average: [3.9, 3.5, 2.8],
                uptime_secs: 41 * 86_400,
            },
        ));
        state.force_sort_containers();
        state.table_state.select(Some(0));
        state
    }

    #[test]
    fn test_system_stats_header() {
        let mut state = create_system_stats_state();
        let buffer = render_list(&mut state, &UiStyles::default(), 150, 20);
        assert_snapshot_with_redaction!(buffer_to_string(&buffer));
    }

    #[test]
    fn test_system_stats_header_narrow() {
        let mut state = create_system_stats_state();
        let buffer = render_list(&mut state, &UiStyles::default(), 100, 20);
        assert_snapshot_with_redaction!(buffer_to_string(&buffer));
    }

    /// `local` sorts after names like `b.example.com`, but its Docker line must
    /// stay under the local machine's System line.
    #[test]
    fn test_system_stats_header_keeps_local_first() {
        let mut state = create_system_stats_state();
        state.handle_event(AppEvent::HostInfo(
            "a-host".to_string(),
            HostInfo {
                cpus: 2,
                memory_total_bytes: 1024 * 1024 * 1024,
            },
        ));
        let buffer = render_list(&mut state, &UiStyles::default(), 150, 20);

        let row = |label: &str| find_text_position(&buffer, label).map(|(_, y)| y);
        assert_eq!(row("workstation"), Some(2));
        assert_eq!(row("Docker  local"), Some(3));
        assert_eq!(row("Docker  a-host"), Some(4));
    }

    /// A failed action used to be dropped silently; it must say what failed.
    #[test]
    fn test_action_error_notification() {
        use crate::core::types::ContainerAction;

        let mut state = create_test_app_state();
        let container = create_test_container("abc123456789", "nginx", "local", 1.0, 1.0, 0.0, 0.0);
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        state.handle_event(AppEvent::ContainerCreated(container));

        state.handle_event(AppEvent::ActionInProgress(
            key.clone(),
            ContainerAction::Stop,
        ));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Stopping nginx…")
        );

        state.handle_event(AppEvent::ActionError(
            key,
            ContainerAction::Stop,
            "Docker responded with status code 500: cannot stop container\nmore detail".to_string(),
        ));
        assert!(state.notification_is_error);
        let buffer = render_list(&mut state, &UiStyles::default(), 120, 15);
        assert_snapshot_with_redaction!(buffer_to_string(&buffer));
    }

    /// Routine progress must not replace an error before it can be read.
    #[test]
    fn test_progress_toast_does_not_replace_an_error() {
        use crate::core::types::ContainerAction;

        let mut state = create_test_app_state();
        let key = ContainerKey::new("local".to_string(), "abc".to_string());
        state.handle_event(AppEvent::ActionError(
            key.clone(),
            ContainerAction::Remove,
            "conflict".to_string(),
        ));
        state.handle_event(AppEvent::ActionSuccess(key.clone(), ContainerAction::Start));

        assert!(state.notification_is_error);
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Failed to remove abc: conflict")
        );

        // A newer error still replaces an older one
        state.handle_event(AppEvent::ActionError(
            key,
            ContainerAction::Stop,
            "timeout".to_string(),
        ));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Failed to stop abc: timeout")
        );
    }

    /// A user-initiated confirmation ("Saved to …") must show even while an
    /// action error is up, unlike background progress.
    #[test]
    fn test_user_notification_replaces_an_error() {
        use crate::core::types::ContainerAction;

        let mut state = create_test_app_state();
        state.handle_event(AppEvent::ActionError(
            ContainerKey::new("local".to_string(), "abc".to_string()),
            ContainerAction::Stop,
            "timeout".to_string(),
        ));
        state.show_notification("Saved to config.yaml");

        assert!(!state.notification_is_error);
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Saved to config.yaml")
        );
    }

    /// Removing a container emits its destroy event, which can drop it from the
    /// list before the action's result arrives; the toast keeps its name.
    #[test]
    fn test_remove_toast_keeps_the_container_name() {
        use crate::core::types::ContainerAction;

        let mut state = create_test_app_state();
        let container = create_test_container("abc123456789", "nginx", "local", 1.0, 1.0, 0.0, 0.0);
        let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
        state.handle_event(AppEvent::ContainerCreated(container));

        state.handle_event(AppEvent::ActionInProgress(
            key.clone(),
            ContainerAction::Remove,
        ));
        state.handle_event(AppEvent::ContainerDestroyed(key.clone()));
        state.handle_event(AppEvent::ActionSuccess(key, ContainerAction::Remove));

        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Removed nginx")
        );
        assert!(state.action_names.is_empty());
    }

    /// An action finishing must not replace the reset prompt while a y/n is armed.
    #[test]
    fn test_action_notification_keeps_reset_prompt() {
        use crate::core::types::ContainerAction;

        let mut state = create_test_app_state();
        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::Char('r'),
            KeyModifiers::CONTROL,
        )));
        state.handle_event(AppEvent::ActionSuccess(
            ContainerKey::new("local".to_string(), "abc".to_string()),
            ContainerAction::Start,
        ));

        assert!(state.reset_confirm_pending);
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Reset all preferences to defaults? (y/n)")
        );
    }

    /// At a common 120 columns, detail is shed but the bars stay.
    #[test]
    fn test_system_stats_header_medium() {
        let mut state = create_system_stats_state();
        let buffer = render_list(&mut state, &UiStyles::default(), 120, 20);
        assert_snapshot_with_redaction!(buffer_to_string(&buffer));
    }

    /// 't' hides the header and gives its rows back to the list.
    #[test]
    fn test_system_stats_header_toggle() {
        let mut state = create_system_stats_state();
        let toggle = || AppEvent::KeyInput(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));

        state.handle_event(toggle());
        assert!(
            !state
                .header_visible
                .load(std::sync::atomic::Ordering::Relaxed)
        );
        let buffer = render_list(&mut state, &UiStyles::default(), 150, 20);
        assert!(find_text_position(&buffer, "System").is_none());
        assert!(find_text_position(&buffer, "nginx").is_some());

        // Shown again: the Docker lines are computed live and come straight
        // back; the machine stats were dropped on hide (they'd be stale) and
        // return with the next sample
        state.handle_event(toggle());
        assert!(
            state
                .header_visible
                .load(std::sync::atomic::Ordering::Relaxed)
        );
        let buffer = render_list(&mut state, &UiStyles::default(), 150, 20);
        assert!(find_text_position(&buffer, "Docker  local").is_some());
        assert!(find_text_position(&buffer, "System").is_none());

        state.handle_event(AppEvent::SystemStats(SystemStats {
            hostname: "workstation".to_string(),
            ..Default::default()
        }));
        let buffer = render_list(&mut state, &UiStyles::default(), 150, 20);
        assert!(find_text_position(&buffer, "System  workstation").is_some());
    }

    /// Docker lines carry no network (container counters double-count traffic
    /// between containers). With no System line to align to, the network
    /// column is left out entirely rather than drawn as an empty gap.
    #[test]
    fn test_docker_only_header_has_no_network_gap() {
        let mut state = create_test_app_state();
        state.handle_event(AppEvent::ContainerCreated(create_test_container(
            "abc123456789",
            "nginx",
            "tcp-host",
            10.0,
            10.0,
            5000.0,
            5000.0,
        )));
        state.handle_event(AppEvent::HostInfo(
            "tcp-host".to_string(),
            HostInfo {
                cpus: 4,
                memory_total_bytes: 8 * 1024 * 1024 * 1024,
            },
        ));
        let buffer = render_list(&mut state, &UiStyles::default(), 150, 20);
        let row: String = (0..150).map(|x| buffer[(x, 2)].symbol()).collect();

        assert!(!row.contains("TX"), "Docker line shows network: {row:?}");
        let after_memory = row.split("/8G").nth(1).unwrap_or_default();
        assert!(
            after_memory.trim_start().starts_with("1/1 running")
                && after_memory.trim_end().len() < 20,
            "gap left where the network column would be: {row:?}"
        );
    }

    /// 80 columns is the classic default; every line must fit without clipping.
    #[test]
    fn test_system_stats_header_80_columns() {
        let mut state = create_system_stats_state();
        let buffer = render_list(&mut state, &UiStyles::default(), 80, 20);
        for y in 2..6 {
            let row: String = (0..80).map(|x| buffer[(x, y)].symbol()).collect();
            assert!(
                row.ends_with(' '),
                "header line {y} reaches the right edge, so it was clipped: {row:?}"
            );
        }
        assert_snapshot_with_redaction!(buffer_to_string(&buffer));
    }

    /// The header gives way on short terminals instead of squeezing out the list.
    #[test]
    fn test_system_stats_header_hidden_when_too_short() {
        let mut state = create_system_stats_state();
        let buffer = render_list(&mut state, &UiStyles::default(), 150, 10);
        assert!(find_text_position(&buffer, "System").is_none());
        assert!(find_text_position(&buffer, "nginx").is_some());
    }

    /// Page up/down must account for the rows the header takes from the list.
    #[test]
    fn test_system_stats_header_shrinks_list_viewport() {
        let mut plain = create_test_app_state();
        render_list(&mut plain, &UiStyles::default(), 150, 20);
        let mut with_header = create_system_stats_state();
        render_list(&mut with_header, &UiStyles::default(), 150, 20);

        // 4 header lines + 1 spacer row
        assert_eq!(
            with_header.last_list_viewport_height,
            plain.last_list_viewport_height - 5
        );
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
                image: None,
                ports: None,
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
                image: None,
                ports: None,
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

    #[test]
    fn test_container_list_with_ports_column() {
        let mut state = create_test_app_state();

        let ports_idx = state
            .column_config
            .columns
            .iter()
            .position(|(c, _)| *c == Column::Ports)
            .unwrap();
        state.column_config.toggle(ports_idx);

        let mut published =
            create_test_container("abc123def456", "nginx", "local", 25.0, 50.0, 1024.0, 2048.0);
        published.ports = Some("53→53/udp, 8080→80".to_string());
        let unpublished =
            create_test_container("def987654321", "worker", "local", 10.0, 20.0, 512.0, 1024.0);

        for container in [published, unpublished] {
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
            state.sorted_container_keys.push(key);
        }
        state.table_state.select(Some(0));

        let styles = UiStyles::default();
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert!(output.contains("Ports"));
        assert!(output.contains("53→53/udp, 8080→80"));
        assert_snapshot_with_redaction!(output);
    }

    #[test]
    fn test_container_list_with_image_column() {
        let mut state = create_test_app_state();

        let image_idx = state
            .column_config
            .columns
            .iter()
            .position(|(c, _)| *c == Column::Image)
            .unwrap();
        state.column_config.toggle(image_idx);

        let mut registry_image =
            create_test_container("def987654321", "api", "local", 10.0, 20.0, 512.0, 1024.0);
        registry_image.image = Some("ghcr.io/acme/api:1.4.2".to_string());
        let mut no_image =
            create_test_container("ghi111222333", "orphan", "local", 5.0, 10.0, 0.0, 0.0);
        no_image.image = None;

        for container in [
            create_test_container("abc123def456", "nginx", "local", 25.0, 50.0, 1024.0, 2048.0),
            registry_image,
            no_image,
        ] {
            let key = ContainerKey::new(container.host_id.clone(), container.id.clone());
            state.containers.insert(key.clone(), container);
            state.sorted_container_keys.push(key);
        }
        state.table_state.select(Some(0));

        let styles = UiStyles::default();
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_ui(f, &mut state, &styles);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let output = buffer_to_string(&buffer);
        assert!(output.contains("Image"));
        assert!(output.contains("nginx:latest"));
        assert!(output.contains("ghcr.io/acme/api:1.4.2"));
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

    /// Sorting by Status puts the containers that need attention first, in the
    /// default direction, instead of ordering state names alphabetically.
    #[test]
    fn test_status_sort_lists_problems_first() {
        let mut state = create_test_app_state();
        state.show_all_containers = true;
        state.sort_state = SortState::new(Column::Status);

        let container = |id: &str, container_state, health| {
            let mut c = create_test_container(id, id, "local", 0.0, 0.0, 0.0, 0.0);
            c.state = container_state;
            c.health = health;
            c
        };
        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![
                container("created", ContainerState::Created, None),
                container(
                    "healthy",
                    ContainerState::Running,
                    Some(HealthStatus::Healthy),
                ),
                container("paused", ContainerState::Paused, None),
                // Stale health on a stopped container must not count as unhealthy.
                container(
                    "exited",
                    ContainerState::Exited,
                    Some(HealthStatus::Unhealthy),
                ),
                container(
                    "starting",
                    ContainerState::Running,
                    Some(HealthStatus::Starting),
                ),
                container("restarting", ContainerState::Restarting, None),
                container(
                    "unhealthy",
                    ContainerState::Running,
                    Some(HealthStatus::Unhealthy),
                ),
                container("dead", ContainerState::Dead, None),
            ],
        ));

        let order: Vec<&str> = state
            .sorted_container_keys
            .iter()
            .map(|k| k.container_id.as_str())
            .collect();
        assert_eq!(
            order,
            [
                "unhealthy",
                "restarting",
                "dead",
                "exited",
                "starting",
                "healthy",
                "paused",
                "created"
            ]
        );
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

    /// Repro for https://github.com/amir20/dtop/issues/362
    ///
    /// While a search filter is active, the last matching container going away
    /// clears the selection. When it comes back the cursor is never restored,
    /// so arrows/Enter do nothing until the filter text is edited.
    #[test]
    fn test_issue_362_selection_lost_when_filtered_container_reappears() {
        let mut state = create_test_app_state();

        let foo = create_test_container("aaa111111111", "foo", "local", 1.0, 1.0, 0.0, 0.0);
        let bar = create_test_container("bbb222222222", "bar", "local", 1.0, 1.0, 0.0, 0.0);
        let foo_key = ContainerKey::new(foo.host_id.clone(), foo.id.clone());

        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![foo.clone(), bar],
        ));

        // Type "/foo"
        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::Char('/'),
            KeyModifiers::NONE,
        )));
        for c in "foo".chars() {
            state.handle_event(AppEvent::KeyInput(KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            )));
        }
        assert_eq!(state.view_state, ViewState::SearchMode);
        assert_eq!(state.sorted_container_keys.len(), 1);
        assert_eq!(state.table_state.selected(), Some(0));

        // `docker compose down` — the only match disappears.
        state.handle_event(AppEvent::ContainerDestroyed(foo_key.clone()));
        assert!(state.sorted_container_keys.is_empty());
        assert_eq!(state.table_state.selected(), None);

        // `docker compose up` — it comes back and matches the filter again.
        state.handle_event(AppEvent::ContainerCreated(foo));
        assert_eq!(
            state.sorted_container_keys.len(),
            1,
            "the filter should match the recreated container"
        );

        assert_eq!(
            state.table_state.selected(),
            Some(0),
            "the recreated container should be selected again"
        );
    }

    /// Companion to the #362 repro: toggling `a` off and back on can empty and
    /// refill the list the same way, and must likewise restore the cursor.
    #[test]
    fn test_toggle_show_all_restores_selection() {
        let mut state = create_test_app_state();
        state.show_all_containers = true;

        let mut stopped = create_test_container("aaa111111111", "foo", "local", 0.0, 0.0, 0.0, 0.0);
        stopped.state = ContainerState::Exited;

        state.handle_event(AppEvent::InitialContainerList(
            "local".to_string(),
            vec![stopped],
        ));
        assert_eq!(state.table_state.selected(), Some(0));

        // 'a' hides the stopped container — the list empties and the cursor goes.
        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
        )));
        assert!(state.sorted_container_keys.is_empty());
        assert_eq!(state.table_state.selected(), None);

        // 'a' again brings it back, and the cursor with it.
        state.handle_event(AppEvent::KeyInput(KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
        )));
        assert_eq!(state.sorted_container_keys.len(), 1);
        assert_eq!(
            state.table_state.selected(),
            Some(0),
            "the restored row should be selected again"
        );
    }

    fn press(state: &mut AppState, code: KeyCode, modifiers: KeyModifiers) {
        state.handle_event(AppEvent::KeyInput(KeyEvent::new(code, modifiers)));
    }

    /// Three containers: `web` and `api` running, `db` exited. Sorted by name
    /// so the rows read api, db, web.
    fn create_bulk_state() -> (AppState, Vec<ContainerKey>) {
        let mut state = create_test_app_state();
        state.show_all_containers = true;
        state.sort_state = SortState::new(Column::Name);
        for (id, name) in [("aaa", "web"), ("bbb", "api"), ("ccc", "db")] {
            let mut container = create_test_container(id, name, "local", 1.0, 1.0, 0.0, 0.0);
            if name == "db" {
                container.state = ContainerState::Exited;
            }
            state.handle_event(AppEvent::ContainerCreated(container));
        }
        let keys = state.sorted_container_keys.clone();
        (state, keys)
    }

    #[test]
    fn test_space_marks_container_and_moves_down() {
        let (mut state, keys) = create_bulk_state();
        state.table_state.select(Some(0));

        press(&mut state, KeyCode::Char(' '), KeyModifiers::NONE);
        assert!(state.marked_containers.contains(&keys[0]));
        assert_eq!(state.table_state.selected(), Some(1));

        // Space on a marked row unmarks it
        state.table_state.select(Some(0));
        press(&mut state, KeyCode::Char(' '), KeyModifiers::NONE);
        assert!(state.marked_containers.is_empty());
    }

    #[test]
    fn test_ctrl_a_toggles_all_and_esc_clears() {
        let (mut state, keys) = create_bulk_state();

        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(state.marked_containers.len(), keys.len());
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert!(state.marked_containers.is_empty());

        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Esc, KeyModifiers::NONE);
        assert!(state.marked_containers.is_empty());
        assert_eq!(state.view_state, ViewState::ContainerList);
    }

    #[test]
    fn test_marks_are_dropped_with_their_container() {
        let (mut state, keys) = create_bulk_state();
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        state.handle_event(AppEvent::ContainerDestroyed(keys[0].clone()));
        assert!(!state.marked_containers.contains(&keys[0]));
        assert_eq!(state.marked_containers.len(), keys.len() - 1);
    }

    /// The bulk menu offers every action at least one marked container
    /// supports, and never Shell.
    #[test]
    fn test_enter_with_marks_opens_bulk_menu() {
        use crate::core::types::ContainerAction;

        let (mut state, _) = create_bulk_state();
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);

        assert_eq!(state.view_state, ViewState::BulkActionMenu);
        assert_eq!(
            state.bulk_actions(),
            vec![
                ContainerAction::Start,
                ContainerAction::Stop,
                ContainerAction::Restart,
                ContainerAction::Remove,
            ]
        );

        let buffer = render_list(&mut state, &UiStyles::default(), 120, 20);
        let output = buffer_to_string(&buffer);
        assert!(output.contains("Actions: 3 selected"), "{output}");
        assert!(output.contains("Stop (2 of 3)"), "{output}");
        assert!(output.contains("Start (1 of 3)"), "{output}");
        assert_snapshot_with_redaction!(output);
    }

    /// Containers hidden by the search filter keep their mark but are not
    /// acted on.
    #[test]
    fn test_bulk_menu_ignores_hidden_marked_containers() {
        let (mut state, _) = create_bulk_state();
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        state.show_all_containers = false;
        state.force_sort_containers();

        assert_eq!(state.marked_visible_keys().len(), 2);
        assert_eq!(state.marked_containers.len(), 3);
    }

    /// Remove in the bulk menu takes a second Enter; moving away disarms it.
    #[test]
    fn test_bulk_remove_needs_confirmation() {
        use crate::core::types::ContainerAction;

        let (mut state, _) = create_bulk_state();
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        for _ in 0..3 {
            press(&mut state, KeyCode::Down, KeyModifiers::NONE);
        }

        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.armed_bulk_action, Some(ContainerAction::Remove));
        assert_eq!(state.view_state, ViewState::BulkActionMenu);
        assert_eq!(state.marked_containers.len(), 3);

        press(&mut state, KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(state.armed_bulk_action, None);

        press(&mut state, KeyCode::Down, KeyModifiers::NONE);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        // No host is connected in tests, so nothing runs, but the menu closes.
        // Only containers an action ran on are unmarked.
        assert_eq!(state.view_state, ViewState::ContainerList);
        assert_eq!(state.marked_containers.len(), 3);
    }

    /// A bulk action reports one summary instead of a toast per container.
    #[test]
    fn test_bulk_progress_summary() {
        use crate::core::app_state::BulkProgress;
        use crate::core::types::ContainerAction;

        let (mut state, keys) = create_bulk_state();
        let running = [keys[0].clone(), keys[2].clone()];
        state.bulk_progress = vec![BulkProgress {
            action: ContainerAction::Stop,
            pending: running.iter().cloned().collect(),
            total: 2,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        }];
        state.show_notification("Stopping 2 containers…");

        state.handle_event(AppEvent::ActionInProgress(
            running[0].clone(),
            ContainerAction::Stop,
        ));
        state.handle_event(AppEvent::ActionSuccess(
            running[0].clone(),
            ContainerAction::Stop,
        ));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Stopping 2 containers…")
        );

        state.handle_event(AppEvent::ActionSuccess(
            running[1].clone(),
            ContainerAction::Stop,
        ));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Stopped 2 containers")
        );
        assert!(state.bulk_progress.is_empty());
    }

    #[test]
    fn test_bulk_progress_summary_reports_failures() {
        use crate::core::app_state::BulkProgress;
        use crate::core::types::ContainerAction;

        let (mut state, keys) = create_bulk_state();
        state.bulk_progress = vec![BulkProgress {
            action: ContainerAction::Restart,
            pending: [keys[0].clone(), keys[2].clone()].into_iter().collect(),
            total: 2,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        }];

        state.handle_event(AppEvent::ActionError(
            keys[0].clone(),
            ContainerAction::Restart,
            "boom".to_string(),
        ));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Failed to restart api: boom")
        );

        state.handle_event(AppEvent::ActionSuccess(
            keys[2].clone(),
            ContainerAction::Restart,
        ));
        assert!(state.notification_is_error);
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Restarted 1 of 2 containers (1 failed: boom)")
        );
    }

    /// With one container, its own error toast says it all; a summary
    /// replacing it would hide the reason.
    #[test]
    fn test_single_bulk_failure_keeps_its_error() {
        use crate::core::app_state::BulkProgress;
        use crate::core::types::ContainerAction;

        let (mut state, keys) = create_bulk_state();
        state.bulk_progress = vec![BulkProgress {
            action: ContainerAction::Stop,
            pending: [keys[0].clone()].into_iter().collect(),
            total: 1,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        }];
        state.handle_event(AppEvent::ActionError(
            keys[0].clone(),
            ContainerAction::Stop,
            "boom".to_string(),
        ));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Failed to stop api: boom")
        );
        assert!(state.bulk_progress.is_empty());
    }

    /// Results from a dropped host may never arrive; the bulk action must not
    /// linger and swallow later toasts for its containers.
    #[test]
    fn test_host_disconnect_clears_bulk_progress() {
        use crate::core::app_state::BulkProgress;
        use crate::core::types::ContainerAction;

        let (mut state, keys) = create_bulk_state();
        state.bulk_progress = vec![BulkProgress {
            action: ContainerAction::Stop,
            pending: [keys[0].clone()].into_iter().collect(),
            total: 2,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        }];
        state.handle_event(AppEvent::HostDisconnected("local".to_string()));
        assert!(state.bulk_progress.is_empty());

        state.handle_event(AppEvent::ActionInProgress(
            keys[0].clone(),
            ContainerAction::Stop,
        ));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Stopping api…")
        );
    }

    #[test]
    fn test_space_on_last_row_marks_without_moving() {
        let (mut state, keys) = create_bulk_state();
        state.table_state.select(Some(keys.len() - 1));
        press(&mut state, KeyCode::Char(' '), KeyModifiers::NONE);
        assert!(state.marked_containers.contains(&keys[keys.len() - 1]));
        assert_eq!(state.table_state.selected(), Some(keys.len() - 1));
    }

    /// Ctrl+A after a search marks only the matching rows.
    #[test]
    fn test_ctrl_a_marks_only_filtered_rows() {
        let (mut state, keys) = create_bulk_state();
        press(&mut state, KeyCode::Char('/'), KeyModifiers::NONE);
        for c in "we".chars() {
            press(&mut state, KeyCode::Char(c), KeyModifiers::NONE);
        }
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.sorted_container_keys.len(), 1);

        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(state.marked_containers.len(), 1);
        assert!(state.marked_containers.contains(&keys[2]));
    }

    /// Marks hidden by a filter are counted in the title, so they aren't
    /// forgotten.
    #[test]
    fn test_title_counts_hidden_marks() {
        let (mut state, _) = create_bulk_state();
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        state.show_all_containers = false;
        state.force_sort_containers();

        let buffer = render_list(&mut state, &UiStyles::default(), 120, 12);
        let output = buffer_to_string(&buffer);
        assert!(
            output.contains("2 containers, 2 selected, 1 hidden"),
            "{output}"
        );
    }

    #[test]
    fn test_marked_rows_render_with_count_in_title() {
        let (mut state, keys) = create_bulk_state();
        state.marked_containers.insert(keys[1].clone());
        let styles = UiStyles::default();
        let buffer = render_list(&mut state, &styles, 120, 12);
        let output = buffer_to_string(&buffer);
        assert!(output.contains("3 containers, 1 selected"), "{output}");

        let (_, y) = find_text_position(&buffer, "db").expect("db row");
        let (x, _) = find_text_position(&buffer, "db").unwrap();
        assert_eq!(buffer[(x, y)].fg, styles.marked.fg.unwrap());
    }

    /// A second bulk action must not orphan the first one's summary.
    #[test]
    fn test_overlapping_bulk_actions_each_summarize() {
        use crate::core::app_state::BulkProgress;
        use crate::core::types::ContainerAction;

        let (mut state, keys) = create_bulk_state();
        let batch = |action, key: &ContainerKey| BulkProgress {
            action,
            pending: [key.clone()].into_iter().collect(),
            total: 1,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        };
        state.bulk_progress = vec![
            batch(ContainerAction::Stop, &keys[0]),
            batch(ContainerAction::Remove, &keys[1]),
        ];

        state.handle_event(AppEvent::ActionSuccess(
            keys[0].clone(),
            ContainerAction::Stop,
        ));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Stopped 1 container")
        );
        state.handle_event(AppEvent::ActionSuccess(
            keys[1].clone(),
            ContainerAction::Remove,
        ));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Removed 1 container")
        );
        assert!(state.bulk_progress.is_empty());
    }

    /// Only one toast is visible at a time, so the summary keeps the latest
    /// reason even when several failed.
    #[test]
    fn test_bulk_summary_with_several_failures_keeps_last_reason() {
        use crate::core::app_state::BulkProgress;
        use crate::core::types::ContainerAction;

        let (mut state, keys) = create_bulk_state();
        state.bulk_progress = vec![BulkProgress {
            action: ContainerAction::Stop,
            pending: keys.iter().cloned().collect(),
            total: 3,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        }];
        for (key, error) in [(&keys[0], "first"), (&keys[1], "second")] {
            state.handle_event(AppEvent::ActionError(
                key.clone(),
                ContainerAction::Stop,
                error.to_string(),
            ));
        }
        state.handle_event(AppEvent::ActionSuccess(
            keys[2].clone(),
            ContainerAction::Stop,
        ));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Stopped 1 of 3 containers (2 failed, last: second)")
        );
    }

    /// A bulk menu whose containers all go away must close, not linger
    /// undrawn with only Esc to escape it.
    #[test]
    fn test_bulk_menu_closes_when_its_containers_go_away() {
        let (mut state, keys) = create_bulk_state();
        state.table_state.select(Some(0));
        press(&mut state, KeyCode::Char(' '), KeyModifiers::NONE);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.view_state, ViewState::BulkActionMenu);
        for _ in 0..3 {
            press(&mut state, KeyCode::Down, KeyModifiers::NONE);
        }

        state.handle_event(AppEvent::ContainerDestroyed(keys[0].clone()));
        assert_eq!(state.view_state, ViewState::ContainerList);
        assert_eq!(state.action_menu_state.selected(), None);
    }

    /// When the menu shrinks, its cursor stays on a real entry.
    #[test]
    fn test_bulk_menu_cursor_is_clamped_when_actions_shrink() {
        let (mut state, keys) = create_bulk_state();
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        for _ in 0..3 {
            press(&mut state, KeyCode::Down, KeyModifiers::NONE);
        }
        // Only `db` (exited) supports Start; it goes away, leaving
        // Stop/Restart/Remove for the two running ones
        state.handle_event(AppEvent::ContainerDestroyed(keys[1].clone()));
        assert_eq!(state.view_state, ViewState::BulkActionMenu);
        assert_eq!(state.action_menu_state.selected(), Some(2));
    }

    /// If the menu reshapes between the two Enters of a bulk Remove, the
    /// second Enter must not run whatever now sits at that index.
    #[test]
    fn test_armed_remove_survives_or_disarms_when_menu_changes() {
        use crate::core::types::ContainerAction;

        let (mut state, keys) = create_bulk_state();
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        for _ in 0..3 {
            press(&mut state, KeyCode::Down, KeyModifiers::NONE);
        }
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.armed_bulk_action, Some(ContainerAction::Remove));

        // Start goes away: the cursor follows Remove up and stays armed
        state.handle_event(AppEvent::ContainerDestroyed(keys[1].clone()));
        assert_eq!(state.selected_menu_action(), Some(ContainerAction::Remove));
        assert_eq!(state.armed_bulk_action, Some(ContainerAction::Remove));

        // An armed action that isn't under the cursor is never confirmed
        state.action_menu_state.select(Some(0));
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.view_state, ViewState::BulkActionMenu);
        assert_eq!(state.armed_bulk_action, None);
    }

    /// Failures are re-marked so they can be retried.
    #[test]
    fn test_failed_bulk_members_are_re_marked() {
        use crate::core::app_state::BulkProgress;
        use crate::core::types::ContainerAction;

        let (mut state, keys) = create_bulk_state();
        state.bulk_progress = vec![BulkProgress {
            action: ContainerAction::Stop,
            pending: [keys[0].clone(), keys[2].clone()].into_iter().collect(),
            total: 2,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now(),
        }];
        state.handle_event(AppEvent::ActionError(
            keys[0].clone(),
            ContainerAction::Stop,
            "boom".to_string(),
        ));
        state.handle_event(AppEvent::ActionSuccess(
            keys[2].clone(),
            ContainerAction::Stop,
        ));
        assert_eq!(
            state.marked_containers,
            [keys[0].clone()].into_iter().collect()
        );
    }

    /// A batch whose results never arrive is dropped, so it can't swallow
    /// later toasts for its containers.
    #[test]
    fn test_stale_bulk_batch_is_dropped() {
        use crate::core::app_state::BulkProgress;
        use crate::core::types::ContainerAction;

        let (mut state, keys) = create_bulk_state();
        state.bulk_progress = vec![BulkProgress {
            action: ContainerAction::Stop,
            pending: [keys[0].clone()].into_iter().collect(),
            total: 1,
            failed: 0,
            last_error: None,
            last_activity: std::time::Instant::now() - std::time::Duration::from_secs(120),
        }];
        state.handle_event(AppEvent::ActionInProgress(
            keys[0].clone(),
            ContainerAction::Stop,
        ));
        assert!(state.bulk_progress.is_empty());
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Stopping api…")
        );
    }

    /// Enter with every mark filtered out says so, rather than silently
    /// opening the single-container menu.
    #[test]
    fn test_enter_with_only_hidden_marks_explains() {
        let (mut state, keys) = create_bulk_state();
        state.marked_containers.insert(keys[1].clone());
        state.show_all_containers = false;
        state.force_sort_containers();
        state.table_state.select(Some(0));

        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.view_state, ViewState::ContainerList);
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Selected containers are hidden; Esc clears the selection")
        );
    }

    /// An armed Remove must not run on a different set of containers than
    /// the one shown when it was armed.
    #[test]
    fn test_armed_remove_disarms_when_its_targets_change() {
        use crate::core::types::ContainerAction;

        let (mut state, keys) = create_bulk_state();
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        for _ in 0..3 {
            press(&mut state, KeyCode::Down, KeyModifiers::NONE);
        }
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.armed_bulk_action, Some(ContainerAction::Remove));

        // `web` goes away; Remove stays in the menu, but now hits 2, not 3
        state.handle_event(AppEvent::ContainerDestroyed(keys[2].clone()));
        assert_eq!(state.selected_menu_action(), Some(ContainerAction::Remove));

        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.view_state, ViewState::BulkActionMenu);
        assert_eq!(state.armed_bulk_action, None);
    }

    #[test]
    fn test_left_cancels_bulk_menu() {
        let (mut state, _) = create_bulk_state();
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut state, KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(state.view_state, ViewState::ContainerList);
        // Marks survive cancelling the menu
        assert_eq!(state.marked_containers.len(), 3);
    }

    /// A stray Esc can drop a large selection, so it says so.
    #[test]
    fn test_esc_clearing_marks_says_so() {
        let (mut state, _) = create_bulk_state();
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Cleared selection of 3 containers")
        );
    }

    #[test]
    fn test_left_cancels_single_action_menu() {
        let (mut state, _) = create_bulk_state();
        state.table_state.select(Some(0));
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert!(matches!(state.view_state, ViewState::ActionMenu(_)));
        press(&mut state, KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(state.view_state, ViewState::ContainerList);
    }

    /// A large batch outlives the timeout as long as its events keep coming;
    /// the timeout measures silence, not age.
    #[test]
    fn test_long_running_bulk_batch_is_kept_while_active() {
        use crate::core::app_state::BulkProgress;
        use crate::core::types::ContainerAction;

        let (mut state, keys) = create_bulk_state();
        state.bulk_progress = vec![BulkProgress {
            action: ContainerAction::Stop,
            pending: [keys[0].clone(), keys[2].clone()].into_iter().collect(),
            total: 2,
            failed: 0,
            last_error: None,
            // Launched long ago, but the first result just arrived
            last_activity: std::time::Instant::now() - std::time::Duration::from_secs(50),
        }];
        state.handle_event(AppEvent::ActionSuccess(
            keys[0].clone(),
            ContainerAction::Stop,
        ));
        // Total age is now well past the timeout from here on, but the
        // result refreshed the batch
        state.bulk_progress[0].last_activity -= std::time::Duration::from_secs(30);
        state.handle_event(AppEvent::ActionSuccess(
            keys[2].clone(),
            ContainerAction::Stop,
        ));
        assert_eq!(
            state.notification.as_ref().map(|(m, _)| m.as_str()),
            Some("Stopped 2 containers")
        );
    }

    /// Rows trading places in a re-sort between the two Enters is the same
    /// target set, so the armed Remove still confirms.
    #[test]
    fn test_armed_remove_survives_a_re_sort() {
        use crate::core::types::{ContainerAction, SortDirection};

        let (mut state, _) = create_bulk_state();
        press(&mut state, KeyCode::Char('a'), KeyModifiers::CONTROL);
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        for _ in 0..3 {
            press(&mut state, KeyCode::Down, KeyModifiers::NONE);
        }
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.armed_bulk_action, Some(ContainerAction::Remove));

        state.sort_state.direction = SortDirection::Descending;
        state.force_sort_containers();

        // No host is connected, so nothing runs, but it was confirmed: the
        // menu closes instead of disarming
        press(&mut state, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.view_state, ViewState::ContainerList);
    }
}
