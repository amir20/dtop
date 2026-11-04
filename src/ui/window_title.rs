//! Sets the terminal's window/tab title (OSC 0) so dtop's tab says what it is
//! watching: `dtop`, `dtop · user@server1`, `dtop · 3 hosts`, or the container
//! whose logs are open. The title from before dtop started is pushed onto the
//! terminal's title stack at startup and popped on exit.

use crate::core::app_state::AppState;
use crate::core::types::ViewState;

/// Saves the current window title on the terminal's title stack (XTWINOPS 22)
pub const PUSH_TITLE: &str = "\x1b[22;0t";
/// Restores the title saved by [`PUSH_TITLE`] (XTWINOPS 23). It is preceded by
/// an empty title, so a terminal without a title stack (e.g. Rex) shows its
/// default title rather than dtop's last one
pub const POP_TITLE: &str = "\x1b]0;\x1b\\\x1b[23;0t";

/// Remembers the title last sent, so it is only written when it changes
#[derive(Default)]
pub struct WindowTitle {
    sent: String,
}

impl WindowTitle {
    /// The escape sequence that sets the title for `state`, or an empty string
    /// when it hasn't changed
    pub fn update(&mut self, state: &AppState) -> String {
        let wanted = title(state);
        if wanted == self.sent {
            return String::new();
        }
        let sequence = format!("\x1b]0;{wanted}\x1b\\");
        self.sent = wanted;
        sequence
    }
}

fn title(state: &AppState) -> String {
    let title = match &state.view_state {
        ViewState::LogView(key) => {
            let name = state
                .containers
                .get(key)
                .map_or(key.container_id.as_str(), |c| c.name.as_str());
            format!("dtop · logs · {name}")
        }
        _ => {
            let mut hosts = state.connected_hosts.keys();
            match (hosts.next(), hosts.next()) {
                (None, _) => "dtop".to_string(),
                (Some(host), None) if host == "local" => "dtop".to_string(),
                (Some(host), None) => format!("dtop · {host}"),
                _ => format!("dtop · {} hosts", state.connected_hosts.len()),
            }
        }
    };
    // Host names come from the config file and could otherwise end the
    // sequence early and inject escapes
    title.chars().filter(|c| !c.is_control()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{
        AppEvent, Column, ColumnConfig, Container, ContainerKey, ContainerState,
    };
    use crate::docker::connection::DockerHost;
    use bollard::{API_DEFAULT_VERSION, Docker};
    use std::collections::HashMap;
    use tokio::sync::mpsc;

    fn state(hosts: &[&str]) -> AppState {
        let (tx, _rx) = mpsc::channel(10);
        let mut state = AppState::new(
            HashMap::new(),
            tx,
            false,
            Column::Uptime,
            None,
            ColumnConfig::default(),
            None,
        );
        for host in hosts {
            // The HTTP client connects lazily, so this never touches a daemon
            let docker =
                Docker::connect_with_http("http://127.0.0.1:1", 1, API_DEFAULT_VERSION).unwrap();
            let host = DockerHost::new(
                host.to_string(),
                docker,
                None,
                HashMap::new(),
                *host == "local",
            );
            state.handle_event(AppEvent::HostConnected(host));
        }
        state
    }

    #[test]
    fn names_what_dtop_is_watching() {
        assert_eq!(title(&state(&["local"])), "dtop");
        assert_eq!(title(&state(&["user@server1"])), "dtop · user@server1");
        assert_eq!(title(&state(&["local", "a", "b"])), "dtop · 3 hosts");
    }

    #[test]
    fn log_view_names_the_container() {
        let mut state = state(&["local"]);
        let key = ContainerKey::new("local".to_string(), "abc123".to_string());
        state.containers.insert(
            key.clone(),
            Container {
                id: "abc123".to_string(),
                name: "web".to_string(),
                state: ContainerState::Running,
                health: None,
                created: None,
                stats: Default::default(),
                host_id: "local".to_string(),
                dozzle_url: None,
                restart_count: None,
                compose_project: None,
                image: None,
                ports: None,
            },
        );
        state.view_state = ViewState::LogView(key);
        assert_eq!(title(&state), "dtop · logs · web");
    }

    #[test]
    fn control_characters_are_dropped() {
        assert_eq!(
            title(&state(&["evil\x1b]0;pwned\x07"])),
            "dtop · evil]0;pwned"
        );
    }

    #[test]
    fn only_writes_changes() {
        let state = state(&["local"]);
        let mut window_title = WindowTitle::default();
        assert_eq!(window_title.update(&state), "\x1b]0;dtop\x1b\\");
        assert_eq!(window_title.update(&state), "");
    }
}
