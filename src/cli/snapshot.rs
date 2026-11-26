//! The plain-text table dtop prints instead of its TUI when stdout isn't a
//! terminal (`dtop | grep nginx`, `dtop > out.txt`).

use bollard::query_parameters::InspectContainerOptions;
use futures_util::stream::{self, StreamExt};
use std::io::Write;
use unicode_width::UnicodeWidthStr;

use crate::cli::config::Config;
use crate::cli::connect::{check_unique_host_ids, connect_and_verify_host};
use crate::core::app_state::sorting::compare_containers;
use crate::core::types::{Column, Container, ContainerState, HealthStatus, SortState};
use crate::docker::connection::{DockerHost, started_at};
use crate::docker::stats::sample_container_stats;
use crate::ui::formatters::{format_bytes_per_sec, format_duration_since, format_time_elapsed};

/// Containers read at once per host. Each read holds a connection (an SSH
/// channel on `ssh://` hosts) for the second or so a stats sample takes; the
/// TUI holds one per running container for the whole session.
const CONCURRENCY_PER_HOST: usize = 16;

/// Spaces between columns, as `docker ps` uses
const COLUMN_GAP: usize = 3;

/// Connects to every configured host, reads its containers once and prints them
/// as a table. A host that can't be reached is reported on stderr and skipped.
pub async fn run(
    config: &Config,
    show_all: bool,
    sort: SortState,
    columns: &[Column],
) -> Result<(), Box<dyn std::error::Error>> {
    check_unique_host_ids(&config.hosts)?;

    let connected: Vec<DockerHost> =
        futures_util::future::join_all(config.hosts.iter().map(connect_and_verify_host))
            .await
            .into_iter()
            .filter_map(|result| result.map_err(|e| eprintln!("dtop: {}", clean(&e))).ok())
            .collect();
    if connected.is_empty() {
        return Err("Failed to connect to any Docker hosts".into());
    }

    let mut containers: Vec<Container> =
        futures_util::future::join_all(connected.iter().map(|host| read_host(host, show_all)))
            .await
            .into_iter()
            .flatten()
            .collect();
    containers.sort_by(|a, b| compare_containers(a, b, sort));

    let table = render_table(&containers, columns, connected.len() > 1);
    match std::io::stdout().lock().write_all(table.as_bytes()) {
        // The reader stopped early (`dtop | head`); that's not an error
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        result => Ok(result?),
    }
}

/// Lists one host's containers and fills in what the list leaves out: start
/// time and restart count from an inspect, and stats for running containers.
async fn read_host(host: &DockerHost, show_all: bool) -> Vec<Container> {
    let listed = match host.list_containers().await {
        Ok(listed) => listed,
        Err(e) => {
            eprintln!(
                "dtop: failed to list containers on '{}': {}",
                clean(&host.host_id),
                clean(&e.to_string())
            );
            return Vec::new();
        }
    };

    stream::iter(
        listed
            .into_iter()
            .filter(|(_, c)| show_all || c.state == ContainerState::Running),
    )
    .map(|(full_id, mut container)| async move {
        if let Ok(inspect) = host
            .docker
            .inspect_container(&full_id, None::<InspectContainerOptions>)
            .await
        {
            container.started = started_at(&inspect);
            container.restart_count = inspect.restart_count;
        }
        if container.state == ContainerState::Running
            && let Some(stats) = sample_container_stats(host, &container.id).await
        {
            container.stats = stats;
        }
        container
    })
    .buffer_unordered(CONCURRENCY_PER_HOST)
    .collect()
    .await
}

/// Lays the containers out as a header row and space-aligned columns, with no
/// colors or escape codes. The last column isn't padded.
fn render_table(containers: &[Container], columns: &[Column], show_host: bool) -> String {
    let columns: Vec<Column> = columns
        .iter()
        .copied()
        .filter(|col| *col != Column::Host || show_host)
        .collect();

    let rows: Vec<Vec<String>> = std::iter::once(columns.iter().map(|c| header(*c)).collect())
        .chain(
            containers
                .iter()
                .map(|container| columns.iter().map(|c| cell(*c, container)).collect()),
        )
        .collect();

    let widths: Vec<usize> = (0..columns.len())
        .map(|i| rows.iter().map(|row| row[i].width()).max().unwrap_or(0))
        .collect();

    let mut out = String::new();
    for row in &rows {
        let mut line = String::new();
        for (i, (text, width)) in row.iter().zip(&widths).enumerate() {
            line.push_str(text);
            if i + 1 < row.len() {
                let pad = width - text.width() + COLUMN_GAP;
                line.extend(std::iter::repeat_n(' ', pad));
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn header(column: Column) -> String {
    match column {
        // The TUI's status column is an unlabelled icon; here it's text
        Column::Status => "Status".to_string(),
        _ => column.label().to_string(),
    }
}

/// One cell as plain text. Stats are blank for stopped containers, like in the TUI.
fn cell(column: Column, container: &Container) -> String {
    let running = container.state == ContainerState::Running;
    let stats = &container.stats;
    match column {
        Column::Status => status_text(container),
        Column::Name => clean(&container.name),
        Column::Id => container.id.clone(),
        Column::Host => clean(&container.host_id),
        Column::Compose => clean(container.compose_project.as_deref().unwrap_or("")),
        Column::Image => clean(container.image.as_deref().unwrap_or("")),
        Column::Ports => clean(container.ports.as_deref().unwrap_or("")),
        _ if !running && column != Column::Created && column != Column::Restarts => String::new(),
        Column::Cpu => format!("{:.1}%", stats.cpu),
        Column::Memory => format!("{:.1}%", stats.memory),
        Column::Pids if stats.pids_limit > 0 => {
            format!("{}/{}", stats.pids_current, stats.pids_limit)
        }
        Column::Pids => stats.pids_current.to_string(),
        Column::NetTx => format_bytes_per_sec(stats.network_tx_bytes_per_sec),
        Column::NetRx => format_bytes_per_sec(stats.network_rx_bytes_per_sec),
        Column::DiskRead => format_bytes_per_sec(stats.disk_read_bytes_per_sec),
        Column::DiskWrite => format_bytes_per_sec(stats.disk_write_bytes_per_sec),
        Column::Uptime => container
            .started
            .map(format_duration_since)
            .unwrap_or_default(),
        Column::Created => format_time_elapsed(container.created.as_ref()),
        Column::Restarts => container
            .restart_count
            .map(|c| c.to_string())
            .unwrap_or_default(),
    }
}

/// `running`, `running (unhealthy)`, `exited`... Health only shows while the
/// container runs: a stopped one keeps its last status, which is stale.
fn status_text(container: &Container) -> String {
    let state = match container.state {
        ContainerState::Running => "running",
        ContainerState::Paused => "paused",
        ContainerState::Restarting => "restarting",
        ContainerState::Removing => "removing",
        ContainerState::Exited => "exited",
        ContainerState::Dead => "dead",
        ContainerState::Created => "created",
        ContainerState::Unknown => "unknown",
    };
    let health = match (&container.state, &container.health) {
        (ContainerState::Running, Some(HealthStatus::Healthy)) => " (healthy)",
        (ContainerState::Running, Some(HealthStatus::Unhealthy)) => " (unhealthy)",
        (ContainerState::Running, Some(HealthStatus::Starting)) => " (starting)",
        _ => "",
    };
    format!("{state}{health}")
}

/// Drops control characters, so a name or a host from a shared config can't
/// write escape sequences into whatever reads the output.
fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::ContainerStats;

    fn container(name: &str, state: ContainerState) -> Container {
        Container {
            id: format!("{name}-id"),
            name: name.to_string(),
            state,
            health: None,
            created: None,
            started: None,
            stats: ContainerStats {
                cpu: 12.34,
                memory: 5.0,
                network_tx_bytes_per_sec: 2048.0,
                ..Default::default()
            },
            host_id: "local".to_string(),
            dozzle_url: None,
            restart_count: Some(2),
            compose_project: None,
            image: Some("nginx:latest".to_string()),
            ports: None,
        }
    }

    #[test]
    fn renders_aligned_plain_text() {
        let mut web = container("web", ContainerState::Running);
        web.health = Some(HealthStatus::Unhealthy);
        let db = container("database", ContainerState::Exited);
        let columns = [
            Column::Status,
            Column::Name,
            Column::Host,
            Column::Cpu,
            Column::NetTx,
            Column::Restarts,
        ];

        let table = render_table(&[web, db], &columns, false);

        assert_eq!(
            table,
            "Status                Name       CPU %   Net TX    Restarts\n\
             running (unhealthy)   web        12.3%   2.0KB/s   2\n\
             exited                database                     2\n"
        );
        assert!(!table.contains('\x1b'));
    }

    #[test]
    fn shows_the_host_column_only_for_several_hosts() {
        let rows = [container("web", ContainerState::Running)];
        let columns = [Column::Name, Column::Host];

        assert_eq!(render_table(&rows, &columns, false), "Name\nweb\n");
        assert_eq!(
            render_table(&rows, &columns, true),
            "Name   Host\nweb    local\n"
        );
    }

    #[test]
    fn strips_control_characters_from_names() {
        let rows = [container("evil\x1b]0;pwned\x07", ContainerState::Running)];

        assert_eq!(
            render_table(&rows, &[Column::Name], false),
            "Name\nevil]0;pwned\n"
        );
    }

    #[test]
    fn hides_health_of_stopped_containers() {
        let mut c = container("web", ContainerState::Exited);
        c.health = Some(HealthStatus::Unhealthy);
        assert_eq!(status_text(&c), "exited");
    }
}
