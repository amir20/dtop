use bollard::query_parameters::{EventsOptions, InspectContainerOptions, ListContainersOptions};
use bollard::{API_DEFAULT_VERSION, Docker};
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::time::Duration;

use crate::core::types::{
    AppEvent, Container, ContainerKey, ContainerState, ContainerStats, EventSender, HostId,
};
use crate::docker::stats::stream_container_stats;

/// Represents a Docker host connection with its identifier
#[derive(Clone, Debug)]
pub struct DockerHost {
    pub host_id: HostId,
    pub docker: Docker,
    pub dozzle_url: Option<String>,
    pub filters: HashMap<String, Vec<String>>,
}

impl DockerHost {
    pub fn new(
        host_id: HostId,
        docker: Docker,
        dozzle_url: Option<String>,
        filters: HashMap<String, Vec<String>>,
    ) -> Self {
        Self {
            host_id,
            docker,
            dozzle_url,
            filters,
        }
    }

    /// Fetches the initial list of containers and starts monitoring them
    async fn fetch_initial_containers(
        &self,
        tx: &EventSender,
        active_containers: &mut HashMap<String, tokio::task::JoinHandle<()>>,
    ) {
        let mut list_options = ListContainersOptions {
            all: true, // Fetch all containers (including stopped ones)
            ..Default::default()
        };

        // Apply filters from DockerHost
        if !self.filters.is_empty() {
            list_options.filters = Some(self.filters.clone());
        }

        let list_options = Some(list_options);

        if let Ok(container_list) = self.docker.list_containers(list_options).await {
            let mut initial_containers = Vec::new();

            for container in container_list {
                let full_id = container.id.clone().unwrap_or_default();
                let truncated_id = full_id[..12.min(full_id.len())].to_string();
                let name = container
                    .names
                    .as_ref()
                    .and_then(|n| n.first().map(|s| s.trim_start_matches('/').to_string()))
                    .unwrap_or_default();
                let state = container
                    .state
                    .as_ref()
                    .and_then(|s| format!("{:?}", s).parse().ok())
                    .unwrap_or(ContainerState::Unknown);

                // Parse created timestamp from Unix timestamp
                let created = container
                    .created
                    .and_then(|timestamp| DateTime::from_timestamp(timestamp, 0));

                // Try to parse health status from Status field
                let health = container
                    .status
                    .as_ref()
                    .and_then(|status| status.parse().ok());

                // Check if container is running before moving state
                let is_running = state == ContainerState::Running;

                let container_info = Container {
                    id: truncated_id.clone(),
                    name: name.clone(),
                    state,
                    health,
                    created,
                    stats: ContainerStats::default(),
                    host_id: self.host_id.clone(),
                    dozzle_url: self.dozzle_url.clone(),
                };

                initial_containers.push(container_info);

                // Only start monitoring for running containers
                if is_running {
                    self.start_container_monitoring(&truncated_id, tx, active_containers);
                }
            }

            // Send all initial containers in one event
            if !initial_containers.is_empty() {
                let _ = tx
                    .send(AppEvent::InitialContainerList(
                        self.host_id.clone(),
                        initial_containers,
                    ))
                    .await;
            }
        }
    }

    /// Monitors Docker events for container start/stop/die events
    async fn monitor_docker_events(
        &self,
        tx: &EventSender,
        active_containers: &mut HashMap<String, tokio::task::JoinHandle<()>>,
    ) {
        // Start with base filters (type and event are always needed)
        let mut filters = HashMap::new();
        filters.insert("type".to_string(), vec!["container".to_string()]);
        filters.insert(
            "event".to_string(),
            vec![
                "start".to_string(),
                "die".to_string(),
                "stop".to_string(),
                "destroy".to_string(),
                "health_status".to_string(),
            ],
        );

        // Merge user-provided filters (only event-compatible ones)
        for (key, values) in &self.filters {
            match key.as_str() {
                // Event-compatible filters
                "container" | "label" | "image" | "network" | "volume" | "daemon" | "scope"
                | "node" | "service" | "secret" | "config" | "plugin" => {
                    filters.insert(key.clone(), values.clone());
                }
                // Map container list filters to event filters where possible
                "id" | "name" => {
                    // For events, id/name should be mapped to "container" filter
                    filters
                        .entry("container".to_string())
                        .or_insert_with(Vec::new)
                        .extend(values.clone());
                }
                // Warn about incompatible filters
                _ => {
                    tracing::warn!(
                        "Filter '{}' is not supported for Docker events API (host: {}). This filter will only apply to container listing.",
                        key,
                        self.host_id
                    );
                }
            }
        }

        let events_options = EventsOptions {
            filters: Some(filters),
            ..Default::default()
        };

        let mut events_stream = self.docker.events(Some(events_options));

        while let Some(event_result) = events_stream.next().await {
            match event_result {
                Ok(event) => {
                    if let Some(actor) = event.actor {
                        let container_id = actor.id.clone().unwrap_or_default();
                        let action = event.action.unwrap_or_default();

                        match action.as_str() {
                            "start" => {
                                self.handle_container_start(&container_id, tx, active_containers)
                                    .await;
                            }
                            "die" | "stop" => {
                                self.handle_container_stop(&container_id, tx, active_containers)
                                    .await;
                            }
                            "destroy" => {
                                self.handle_container_destroy(&container_id, tx, active_containers)
                                    .await;
                            }
                            "health_status"
                            | "health_status: healthy"
                            | "health_status: unhealthy" => {
                                self.handle_health_status_change(&container_id, &actor, tx)
                                    .await;
                            }
                            _ => {}
                        }
                    }
                }
                Err(_) => {
                    // If event stream fails, wait and continue
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// Starts monitoring a container by spawning a stats stream task
    fn start_container_monitoring(
        &self,
        truncated_id: &str,
        tx: &EventSender,
        active_containers: &mut HashMap<String, tokio::task::JoinHandle<()>>,
    ) {
        let tx_clone = tx.clone();
        let host_clone = self.clone();
        let truncated_id_clone = truncated_id.to_string();

        let handle = tokio::spawn(async move {
            stream_container_stats(host_clone, truncated_id_clone, tx_clone).await;
        });

        active_containers.insert(truncated_id.to_string(), handle);
    }

    /// Handles a container start event
    async fn handle_container_start(
        &self,
        container_id: &str,
        tx: &EventSender,
        active_containers: &mut HashMap<String, tokio::task::JoinHandle<()>>,
    ) {
        let truncated_id = container_id[..12.min(container_id.len())].to_string();

        // Get container details
        if let Ok(inspect) = self
            .docker
            .inspect_container(container_id, None::<InspectContainerOptions>)
            .await
        {
            let name = inspect
                .name
                .as_ref()
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_default();

            let state = inspect
                .state
                .as_ref()
                .and_then(|s| s.status.as_ref())
                .and_then(|s| format!("{:?}", s).parse().ok())
                .unwrap_or(ContainerState::Unknown);

            // Parse health status from state (None if no health check configured)
            let health = inspect
                .state
                .as_ref()
                .and_then(|s| s.health.as_ref())
                .and_then(|h| h.status.as_ref())
                .and_then(|status| format!("{:?}", status).parse().ok());

            // Parse created timestamp from RFC3339 string
            let created = inspect.created.as_ref().and_then(|created_str| {
                DateTime::parse_from_rfc3339(created_str)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });

            // Start monitoring the new container
            if !active_containers.contains_key(&truncated_id) {
                let container = Container {
                    id: truncated_id.clone(),
                    name: name.clone(),
                    state,
                    health,
                    created,
                    stats: ContainerStats::default(),
                    host_id: self.host_id.clone(),
                    dozzle_url: self.dozzle_url.clone(),
                };

                let _ = tx.send(AppEvent::ContainerCreated(container)).await;

                self.start_container_monitoring(&truncated_id, tx, active_containers);
            }
        }
    }

    /// Handles a container stop/die event
    async fn handle_container_stop(
        &self,
        container_id: &str,
        tx: &EventSender,
        active_containers: &mut HashMap<String, tokio::task::JoinHandle<()>>,
    ) {
        let truncated_id = container_id[..12.min(container_id.len())].to_string();

        // Stop stats monitoring but keep the container in the list
        if let Some(handle) = active_containers.remove(&truncated_id) {
            handle.abort();

            // Send state change event instead of destroying the container
            let key = ContainerKey::new(self.host_id.clone(), truncated_id);
            let _ = tx
                .send(AppEvent::ContainerStateChanged(key, ContainerState::Exited))
                .await;
        }
    }

    /// Handles a container destroy event (when container is actually removed)
    async fn handle_container_destroy(
        &self,
        container_id: &str,
        tx: &EventSender,
        active_containers: &mut HashMap<String, tokio::task::JoinHandle<()>>,
    ) {
        let truncated_id = container_id[..12.min(container_id.len())].to_string();

        // Stop monitoring if still active and remove from UI
        if let Some(handle) = active_containers.remove(&truncated_id) {
            handle.abort();
        }

        let key = ContainerKey::new(self.host_id.clone(), truncated_id);
        let _ = tx.send(AppEvent::ContainerDestroyed(key)).await;
    }

    /// Handles a health_status event
    async fn handle_health_status_change(
        &self,
        container_id: &str,
        actor: &bollard::models::EventActor,
        tx: &EventSender,
    ) {
        let truncated_id = container_id[..12.min(container_id.len())].to_string();

        // Try to get health status from actor attributes
        let health = if let Some(attributes) = &actor.attributes {
            attributes
                .get("health_status")
                .or_else(|| attributes.get("HealthStatus"))
                .and_then(|status| status.parse().ok())
        } else {
            // Fallback: inspect the container to get current health status
            if let Ok(inspect) = self
                .docker
                .inspect_container(container_id, None::<InspectContainerOptions>)
                .await
            {
                inspect
                    .state
                    .as_ref()
                    .and_then(|s| s.health.as_ref())
                    .and_then(|h| h.status.as_ref())
                    .and_then(|status| format!("{:?}", status).parse().ok())
            } else {
                None
            }
        };

        // Only send event if we have a valid health status
        if let Some(health_status) = health {
            let key = ContainerKey::new(self.host_id.clone(), truncated_id);
            let _ = tx
                .send(AppEvent::ContainerHealthChanged(key, health_status))
                .await;
        }
    }

    /// Starts a container
    pub async fn start_container(&self, container_id: &str) -> Result<(), String> {
        use bollard::query_parameters::StartContainerOptions;

        let options = StartContainerOptions { detach_keys: None };

        self.docker
            .start_container(container_id, Some(options))
            .await
            .map_err(|e| format!("Failed to start container: {}", e))
    }

    /// Stops a container with a 10-second timeout
    pub async fn stop_container(&self, container_id: &str) -> Result<(), String> {
        use bollard::query_parameters::StopContainerOptions;

        let options = StopContainerOptions {
            signal: None,
            t: Some(10), // 10 second timeout before force kill
        };

        self.docker
            .stop_container(container_id, Some(options))
            .await
            .map_err(|e| format!("Failed to stop container: {}", e))
    }

    /// Restarts a container with a 10-second timeout
    pub async fn restart_container(&self, container_id: &str) -> Result<(), String> {
        use bollard::query_parameters::RestartContainerOptions;

        let options = RestartContainerOptions {
            signal: None,
            t: Some(10), // 10 second timeout before force kill
        };

        self.docker
            .restart_container(container_id, Some(options))
            .await
            .map_err(|e| format!("Failed to restart container: {}", e))
    }

    /// Removes a container (with force option if needed)
    pub async fn remove_container(&self, container_id: &str) -> Result<(), String> {
        use bollard::query_parameters::RemoveContainerOptions;

        let options = RemoveContainerOptions {
            force: true, // Force removal even if running
            v: false,    // Don't remove volumes
            link: false,
        };

        self.docker
            .remove_container(container_id, Some(options))
            .await
            .map_err(|e| format!("Failed to remove container: {}", e))
    }

    /// Runs an interactive shell session inside a container
    /// This function takes over the terminal completely until the shell exits
    pub async fn run_shell_session(
        &self,
        container_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        crate::docker::shell::run_shell_session(self, container_id).await
    }
}

/// Manages container monitoring for a specific Docker host: fetches initial containers and listens for Docker events
pub async fn container_manager(host: DockerHost, tx: EventSender) {
    let mut active_containers: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();

    // Fetch and start monitoring initial containers
    host.fetch_initial_containers(&tx, &mut active_containers)
        .await;

    // Subscribe to Docker events and handle container lifecycle
    host.monitor_docker_events(&tx, &mut active_containers)
        .await;
}

/// Connects to Docker based on the host string
///
/// # Arguments
/// * `host` - Host specification string (e.g., "local", "ssh://user@host", "tcp://host:port", "tls://host:port")
///
/// # Returns
/// * `Ok(Docker)` - Successfully connected Docker instance
/// * `Err` - Connection error with details
///
/// # Examples
/// ```ignore
/// let docker = connect_docker("local")?;
/// let docker = connect_docker("ssh://user@host")?;
/// let docker = connect_docker("tcp://host:2375")?;
/// let docker = connect_docker("tls://host:2376")?;
/// ```
pub fn connect_docker(host: &str) -> Result<Docker, Box<dyn std::error::Error>> {
    use tracing::{debug, error};

    if host == "local" {
        debug!("Connecting to local Docker daemon");
        // Connect to local Docker daemon using default settings
        Docker::connect_with_local_defaults().map_err(|e| {
            error!("Local Docker connection failed: {:?}", e);
            e.into()
        })
    } else if host.starts_with("ssh://") {
        debug!("Connecting to Docker via SSH: {}", host);
        debug!(
            "SSH timeout: 120 seconds, API version: {}",
            API_DEFAULT_VERSION
        );

        // Connect via SSH with 120 second timeout
        Docker::connect_with_ssh(
            host,
            120, // timeout in seconds
            API_DEFAULT_VERSION,
            None, // no custom socket path
        )
        .map_err(|e| {
            error!("SSH Docker connection failed for '{}': {:?}", host, e);
            debug!("Bollard SSH error type: {}", std::any::type_name_of_val(&e));
            e.into()
        })
    } else if host.starts_with("tls://") {
        // Connect via TLS using environment variables for certificates
        // Expects DOCKER_CERT_PATH to be set with key.pem, cert.pem, and ca.pem files
        let cert_path = std::env::var("DOCKER_CERT_PATH")
            .unwrap_or_else(|_| format!("{}/.docker", std::env::var("HOME").unwrap_or_default()));

        let cert_dir = std::path::Path::new(&cert_path);
        let key_path = cert_dir.join("key.pem");
        let cert_path = cert_dir.join("cert.pem");
        let ca_path = cert_dir.join("ca.pem");

        // Convert tls:// to tcp:// for Bollard
        let tcp_host = host.replace("tls://", "tcp://");

        Ok(Docker::connect_with_ssl(
            &tcp_host,
            &key_path,
            &cert_path,
            &ca_path,
            120, // timeout in seconds
            API_DEFAULT_VERSION,
        )?)
    } else if host.starts_with("tcp://") {
        // Connect via TCP (remote Docker daemon)
        Ok(Docker::connect_with_http(
            host,
            120, // timeout in seconds
            API_DEFAULT_VERSION,
        )?)
    } else {
        Err(format!(
            "Invalid host format: '{}'. Use 'local', 'ssh://user@host[:port]', 'tcp://host:port', or 'tls://host:port'",
            host
        )
        .into())
    }
}
