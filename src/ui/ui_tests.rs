#[cfg(test)]
mod tests {
    use crate::core::app_state::AppState;
    use crate::core::types::{Container, ContainerKey, ContainerState, ContainerStats, ViewState};
    use crate::ui::render::{UiStyles, render_ui};
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
        AppState::new(HashMap::new(), tx)
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
            },
            host_id: host_id.to_string(),
            dozzle_url: None,
        }
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

        let backend = TestBackend::new(140, 25);
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
        state.current_log_container = Some(key);
        state.is_at_bottom = true;

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
        state.current_log_container = Some(key);
        state.is_at_bottom = true;

        // Manually create formatted log text (simulating what would come from log entries)
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span, Text};

        let timestamp_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let lines = vec![
            Line::from(vec![
                Span::styled("2025-10-29T10:15:30Z", timestamp_style),
                Span::raw(" Server started on port 80"),
            ]),
            Line::from(vec![
                Span::styled("2025-10-29T10:15:31Z", timestamp_style),
                Span::raw(" Accepting connections"),
            ]),
            Line::from(vec![
                Span::styled("2025-10-29T10:15:32Z", timestamp_style),
                Span::raw(" GET /health 200 OK"),
            ]),
            Line::from(vec![
                Span::styled("2025-10-29T10:15:33Z", timestamp_style),
                Span::raw(" GET /api/users 200 OK"),
            ]),
        ];
        state.formatted_log_text = Text::from(lines);

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
        state.current_log_container = Some(key);
        state.is_at_bottom = false; // Manual scroll mode
        state.log_scroll_offset = 5;

        // Add log content
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span, Text};

        let timestamp_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let lines = vec![
            Line::from(vec![
                Span::styled("2025-10-29T10:15:30Z", timestamp_style),
                Span::raw(" Log line 1"),
            ]),
            Line::from(vec![
                Span::styled("2025-10-29T10:15:31Z", timestamp_style),
                Span::raw(" Log line 2"),
            ]),
            Line::from(vec![
                Span::styled("2025-10-29T10:15:32Z", timestamp_style),
                Span::raw(" Log line 3"),
            ]),
        ];
        state.formatted_log_text = Text::from(lines);

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
}
