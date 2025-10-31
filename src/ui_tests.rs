#[cfg(test)]
mod tests {
    use crate::app_state::AppState;
    use crate::types::{Container, ContainerKey, ContainerState, ContainerStats, ViewState};
    use crate::ui::{UiStyles, render_ui};
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
}
