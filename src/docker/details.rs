use bollard::models::ContainerInspectResponse;
use bollard::query_parameters::InspectContainerOptions;
use chrono::{DateTime, Datelike, Utc};

use crate::core::types::{AppEvent, ContainerDetails, ContainerKey, EventSender};
use crate::docker::connection::DockerHost;

/// Inspects a container for the details popup and sends the result back.
pub async fn fetch_container_details(host: DockerHost, key: ContainerKey, tx: EventSender) {
    let result = host
        .docker
        .inspect_container(&key.container_id, None::<InspectContainerOptions>)
        .await
        .map(details_from_inspect)
        .map_err(|e| e.to_string());
    let _ = tx.send(AppEvent::ContainerDetails(key, result)).await;
}

/// Docker reports timestamps that never happened (a container that has not
/// stopped yet) as the zero time, `0001-01-01T00:00:00Z`.
fn parse_time(value: Option<&String>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value?)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .filter(|dt| dt.year() > 1)
}

fn details_from_inspect(inspect: ContainerInspectResponse) -> ContainerDetails {
    let state = inspect.state.as_ref();
    let config = inspect.config.as_ref();

    let command = inspect
        .path
        .iter()
        .chain(inspect.args.iter().flatten())
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");

    let restart_policy = inspect
        .host_config
        .as_ref()
        .and_then(|hc| hc.restart_policy.as_ref())
        .map(|policy| {
            let name = policy.name.map(|n| n.to_string()).unwrap_or_default();
            let name = if name.is_empty() {
                "no".to_string()
            } else {
                name
            };
            match policy.maximum_retry_count {
                Some(max) if max > 0 => format!("{name} (max {max})"),
                _ => name,
            }
        })
        .unwrap_or_else(|| "no".to_string());

    let network_settings = inspect.network_settings.as_ref();
    // The live bindings are only reported while the container runs; a stopped
    // one falls back to what it was created with.
    let mut ports: Vec<String> = network_settings
        .and_then(|ns| ns.ports.as_ref())
        .filter(|ports| !ports.is_empty())
        .or_else(|| {
            inspect
                .host_config
                .as_ref()
                .and_then(|hc| hc.port_bindings.as_ref())
        })
        .into_iter()
        .flatten()
        .flat_map(|(spec, bindings)| {
            bindings.iter().flatten().filter_map(move |binding| {
                let host_port = binding.host_port.as_deref().filter(|p| !p.is_empty())?;
                let host_ip = binding.host_ip.as_deref().unwrap_or("");
                // spec is "<container port>/<protocol>"
                let (port, protocol) = spec.split_once('/').unwrap_or((spec, "tcp"));
                Some(if host_ip.is_empty() {
                    format!("{host_port}→{port}/{protocol}")
                } else if host_ip.contains(':') {
                    format!("[{host_ip}]:{host_port}→{port}/{protocol}")
                } else {
                    format!("{host_ip}:{host_port}→{port}/{protocol}")
                })
            })
        })
        .collect();
    ports.sort();

    let mut mounts: Vec<String> = inspect
        .mounts
        .iter()
        .flatten()
        .map(|mount| {
            // Named volumes read better by name than by their path under /var/lib/docker
            let source = mount
                .name
                .as_deref()
                .or(mount.source.as_deref())
                .unwrap_or("");
            let destination = mount.destination.as_deref().unwrap_or("");
            let read_only = if mount.rw == Some(false) { " (ro)" } else { "" };
            format!("{source} → {destination}{read_only}")
        })
        .collect();
    mounts.sort();

    let mut networks: Vec<String> = network_settings
        .and_then(|ns| ns.networks.as_ref())
        .into_iter()
        .flatten()
        .map(|(name, endpoint)| match endpoint.ip_address.as_deref() {
            Some(ip) if !ip.is_empty() => format!("{name} {ip}"),
            _ => name.clone(),
        })
        .collect();
    networks.sort();

    ContainerDetails {
        image: config
            .and_then(|c| c.image.clone())
            .or_else(|| inspect.image.clone())
            .unwrap_or_default(),
        command,
        created: parse_time(inspect.created.as_ref()),
        started: parse_time(state.and_then(|s| s.started_at.as_ref())),
        finished: parse_time(state.and_then(|s| s.finished_at.as_ref())),
        exit_code: state.and_then(|s| s.exit_code),
        oom_killed: state.and_then(|s| s.oom_killed).unwrap_or(false),
        error: state
            .and_then(|s| s.error.clone())
            .filter(|e| !e.is_empty()),
        restart_policy,
        restart_count: inspect.restart_count.unwrap_or(0),
        ports,
        mounts,
        networks,
        label_count: config
            .and_then(|c| c.labels.as_ref())
            .map_or(0, |labels| labels.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trimmed `docker inspect` of a container that was OOM-killed.
    const INSPECT: &str = r#"{
        "Created": "2026-10-14T09:00:00.123Z",
        "Path": "/docker-entrypoint.sh",
        "Args": ["nginx", "-g", "daemon off;"],
        "State": {
            "Status": "exited", "ExitCode": 137, "OOMKilled": true, "Error": "",
            "StartedAt": "2026-10-14T09:00:01Z", "FinishedAt": "2026-10-15T10:30:00Z"
        },
        "Image": "sha256:abc",
        "RestartCount": 2,
        "HostConfig": { "RestartPolicy": { "Name": "on-failure", "MaximumRetryCount": 3 } },
        "Mounts": [
            { "Type": "bind", "Source": "/srv/conf", "Destination": "/etc/nginx", "RW": false },
            { "Type": "volume", "Name": "web-data", "Source": "/var/lib/docker/volumes/web-data/_data",
              "Destination": "/data", "RW": true }
        ],
        "Config": { "Image": "nginx:alpine", "Labels": { "a": "1", "b": "2" } },
        "NetworkSettings": {
            "Ports": {
                "80/tcp": [ { "HostIp": "0.0.0.0", "HostPort": "8080" }, { "HostIp": "::", "HostPort": "8080" } ],
                "443/tcp": null
            },
            "Networks": { "web_default": { "IPAddress": "172.18.0.2" }, "none": { "IPAddress": "" } }
        }
    }"#;

    #[test]
    fn details_from_inspect_reads_every_field() {
        let inspect: ContainerInspectResponse = serde_json::from_str(INSPECT).unwrap();
        let details = details_from_inspect(inspect);

        assert_eq!(details.image, "nginx:alpine");
        assert_eq!(
            details.command,
            "/docker-entrypoint.sh nginx -g daemon off;"
        );
        assert_eq!(
            details.created.unwrap().to_rfc3339(),
            "2026-10-14T09:00:00.123+00:00"
        );
        assert!(details.started.is_some());
        assert!(details.finished.is_some());
        assert_eq!(details.exit_code, Some(137));
        assert!(details.oom_killed);
        assert_eq!(details.error, None);
        assert_eq!(details.restart_policy, "on-failure (max 3)");
        assert_eq!(details.restart_count, 2);
        // An exposed port with no binding (443) is not published
        assert_eq!(details.ports, ["0.0.0.0:8080→80/tcp", "[::]:8080→80/tcp"]);
        assert_eq!(
            details.mounts,
            ["/srv/conf → /etc/nginx (ro)", "web-data → /data"]
        );
        assert_eq!(details.networks, ["none", "web_default 172.18.0.2"]);
        assert_eq!(details.label_count, 2);
    }

    #[test]
    fn zero_time_means_never() {
        let inspect: ContainerInspectResponse = serde_json::from_str(
            r#"{ "State": { "StartedAt": "2026-10-14T09:00:01Z", "FinishedAt": "0001-01-01T00:00:00Z" } }"#,
        )
        .unwrap();
        let details = details_from_inspect(inspect);
        assert!(details.started.is_some());
        assert_eq!(details.finished, None);
        assert_eq!(details.restart_policy, "no");
    }

    #[test]
    fn stopped_container_falls_back_to_configured_port_bindings() {
        let inspect: ContainerInspectResponse = serde_json::from_str(
            r#"{
                "HostConfig": { "PortBindings": { "80/tcp": [ { "HostIp": "", "HostPort": "18081" } ] } },
                "NetworkSettings": { "Ports": {} }
            }"#,
        )
        .unwrap();
        assert_eq!(details_from_inspect(inspect).ports, ["18081→80/tcp"]);
    }
}
