use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use url::Url;

use crate::cli::config::{Config, HostConfig};
use crate::cli::filters::parse_filters;
use crate::core::types::AppEvent;
use crate::docker::connection::{DockerHost, connect_docker, container_manager};

/// Result of establishing connections to Docker hosts
pub struct ConnectionResult {
    /// The first successfully connected host
    pub first_host: DockerHost,
    /// Receiver for additional hosts that connect after the first
    pub remaining_rx: mpsc::Receiver<DockerHost>,
}

/// Establishes connections to all configured Docker hosts in parallel.
/// Returns as soon as the first host connects successfully.
/// Remaining connections continue in the background.
pub async fn establish_connections(
    config: &Config,
    event_tx: mpsc::Sender<AppEvent>,
) -> Result<ConnectionResult, Box<dyn std::error::Error>> {
    let total_hosts = config.hosts.len();

    // Create a channel for receiving successful connections
    let (conn_tx, mut conn_rx) = mpsc::channel::<DockerHost>(total_hosts);

    // Spawn all connection attempts in parallel
    let connection_handles: Vec<_> = config
        .hosts
        .iter()
        .map(|host_config| {
            let host_config = host_config.clone();
            let conn_tx = conn_tx.clone();
            let error_tx = event_tx.clone();

            tokio::spawn(async move {
                match connect_and_verify_host(&host_config).await {
                    Ok(docker_host) => {
                        let _ = conn_tx.send(docker_host).await;
                    }
                    Err(e) => {
                        use tracing::error;
                        error!("{}", e);

                        // Create host_id for the error event
                        let host_id = create_host_id(&host_config.host);

                        // Send error event to UI
                        let _ = error_tx
                            .send(AppEvent::ConnectionError(host_id, e.clone()))
                            .await;

                        if total_hosts == 1 {
                            eprintln!("Failed to connect to Docker host: {:?}", e);
                        }
                    }
                }
            })
        })
        .collect();

    // Drop the original sender so the channel closes when all tasks complete
    drop(conn_tx);

    // Try to get the first connection with a reasonable timeout
    let first_host = match tokio::time::timeout(Duration::from_secs(30), conn_rx.recv()).await {
        Ok(Some(docker_host)) => {
            use tracing::debug;

            if total_hosts > 1 {
                debug!("Connected to host 1/{}, starting UI...", total_hosts);
            }

            docker_host
        }
        Ok(None) => {
            // Channel closed without any connections
            return Err("Failed to connect to any Docker hosts. Please check your configuration and connection settings. Set DEBUG=1 to see detailed logs in debug.log".into());
        }
        Err(_) => {
            // Timeout waiting for first connection
            return Err("Timeout waiting for Docker host connections (30s). Please check your network and Docker daemon status.".into());
        }
    };

    // Create a new channel to forward remaining connections
    let (remaining_tx, remaining_rx) = mpsc::channel::<DockerHost>(total_hosts);

    // Spawn task to collect remaining connections and forward them
    tokio::spawn(async move {
        use tracing::debug;
        let mut remaining_count = 1; // Already got one

        while let Some(docker_host) = conn_rx.recv().await {
            let _ = remaining_tx.send(docker_host).await;
            remaining_count += 1;
            if total_hosts > 1 {
                debug!("Connected to host {}/{}", remaining_count, total_hosts);
            }
        }

        // Wait for all connection attempts to complete
        for handle in connection_handles {
            let _ = handle.await;
        }
    });

    Ok(ConnectionResult {
        first_host,
        remaining_rx,
    })
}

/// Spawns background task to handle remaining host connections
pub fn spawn_remaining_connections_handler(
    mut remaining_rx: mpsc::Receiver<DockerHost>,
    event_tx: mpsc::Sender<AppEvent>,
) {
    tokio::spawn(async move {
        while let Some(docker_host) = remaining_rx.recv().await {
            // Send HostConnected event so AppState can track this host for log streaming
            let _ = event_tx
                .send(AppEvent::HostConnected(docker_host.clone()))
                .await;

            // Spawn container manager for this host
            let tx_clone = event_tx.clone();
            tokio::spawn(async move {
                container_manager(docker_host, tx_clone).await;
            });
        }
    });
}

/// Connects to a Docker host and verifies the connection works
/// Returns Ok(DockerHost) if successful, Err with details if connection fails
pub async fn connect_and_verify_host(host_config: &HostConfig) -> Result<DockerHost, String> {
    use tracing::debug;

    let host_spec = &host_config.host;

    debug!("Attempting to connect to host: {}", host_spec);

    // Attempt to connect
    let docker = connect_docker(host_spec).map_err(|e| {
        format!(
            "Failed to create Docker client for host '{}': {}",
            host_spec, e
        )
    })?;

    debug!("Successfully created Docker client for host: {}", host_spec);

    // Parse filters if provided
    let filters = if let Some(ref filter_list) = host_config.filter {
        parse_filters(filter_list)
            .map_err(|e| format!("Failed to parse filters for host '{}': {}", host_spec, e))?
    } else {
        HashMap::new()
    };

    // Create host ID and DockerHost instance
    let host_id = create_host_id(host_spec);
    let docker_host = DockerHost::new(host_id, docker, host_config.dozzle.clone(), filters);

    // Verify the connection actually works by pinging Docker with timeout
    debug!("Pinging Docker daemon at host: {}", host_spec);
    let ping_timeout = Duration::from_secs(10);

    match tokio::time::timeout(ping_timeout, docker_host.docker.ping()).await {
        Ok(Ok(_)) => {
            debug!("Successfully pinged Docker daemon at host: {}", host_spec);
            Ok(docker_host)
        }
        Ok(Err(e)) => {
            debug!("Ping error details: {:?}", e);
            debug!("Error source chain:");
            for (level, err) in std::iter::successors(std::error::Error::source(&e), |e| {
                std::error::Error::source(*e)
            })
            .enumerate()
            {
                debug!("  Level {}: {}", level + 1, err);
            }
            Err(format!(
                "Docker daemon ping failed for host '{}': {}",
                host_spec, e
            ))
        }
        Err(_) => Err(format!(
            "Docker daemon ping timeout for host '{}' (>10s)",
            host_spec
        )),
    }
}

/// Creates a unique host identifier from the host specification
pub fn create_host_id(host_spec: &str) -> String {
    if host_spec == "local" {
        "local".to_string()
    } else if let Ok(url) = Url::parse(host_spec) {
        // Extract just the domain/host from the URL
        url.host_str().unwrap_or(host_spec).to_string()
    } else {
        host_spec.to_string()
    }
}
