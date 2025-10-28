use bollard::Docker;
use bollard::query_parameters::{EventsOptions, InspectContainerOptions, ListContainersOptions};
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::time::Duration;

use crate::stats::stream_container_stats;
use crate::types::{AppEvent, Container, ContainerKey, ContainerStats, EventSender, HostId};

/// Represents a Docker host connection with its identifier
#[derive(Clone)]
pub struct DockerHost {
    pub host_id: HostId,
    pub docker: Docker,
}

impl DockerHost {
    pub fn new(host_id: HostId, docker: Docker) -> Self {
        Self { host_id, docker }
    }
}

/// Manages container monitoring for a specific Docker host: fetches initial containers and listens for Docker events
pub async fn container_manager(host: DockerHost, tx: EventSender) {
    let mut active_containers: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();

    // Fetch and start monitoring initial containers
    fetch_initial_containers(&host, &tx, &mut active_containers).await;

    // Subscribe to Docker events and handle container lifecycle
    monitor_docker_events(&host, &tx, &mut active_containers).await;
}

/// Fetches the initial list of running containers and starts monitoring them
async fn fetch_initial_containers(
    host: &DockerHost,
    tx: &EventSender,
    active_containers: &mut HashMap<String, tokio::task::JoinHandle<()>>,
) {
    let list_options = Some(ListContainersOptions {
        all: false,
        ..Default::default()
    });

    if let Ok(container_list) = host.docker.list_containers(list_options).await {
        let mut initial_containers = Vec::new();

        for container in container_list {
            let full_id = container.id.clone().unwrap_or_default();
            let truncated_id = full_id[..12.min(full_id.len())].to_string();
            let name = container
                .names
                .as_ref()
                .and_then(|n| n.first().map(|s| s.trim_start_matches('/').to_string()))
                .unwrap_or_default();
            let status = container.status.clone().unwrap_or_default();

            let container_info = Container {
                id: truncated_id.clone(),
                name: name.clone(),
                status: status.clone(),
                stats: ContainerStats::default(),
                host_id: host.host_id.clone(),
            };

            initial_containers.push(container_info);

            start_container_monitoring(host, &truncated_id, tx, active_containers);
        }

        // Send all initial containers in one event
        if !initial_containers.is_empty() {
            let _ = tx
                .send(AppEvent::InitialContainerList(
                    host.host_id.clone(),
                    initial_containers,
                ))
                .await;
        }
    }
}

/// Monitors Docker events for container start/stop/die events
async fn monitor_docker_events(
    host: &DockerHost,
    tx: &EventSender,
    active_containers: &mut HashMap<String, tokio::task::JoinHandle<()>>,
) {
    let mut filters = HashMap::new();
    filters.insert("type".to_string(), vec!["container".to_string()]);
    filters.insert(
        "event".to_string(),
        vec!["start".to_string(), "die".to_string(), "stop".to_string()],
    );

    let events_options = EventsOptions {
        filters: Some(filters),
        ..Default::default()
    };

    let mut events_stream = host.docker.events(Some(events_options));

    while let Some(event_result) = events_stream.next().await {
        match event_result {
            Ok(event) => {
                if let Some(actor) = event.actor {
                    let container_id = actor.id.unwrap_or_default();
                    let action = event.action.unwrap_or_default();

                    match action.as_str() {
                        "start" => {
                            handle_container_start(host, &container_id, tx, active_containers)
                                .await;
                        }
                        "die" | "stop" => {
                            handle_container_stop(host, &container_id, tx, active_containers).await;
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
///
/// # Arguments
/// * `host` - Docker host instance with identifier
/// * `truncated_id` - Truncated container ID (12 chars)
/// * `tx` - Event sender channel
/// * `active_containers` - Map of active container monitoring tasks
fn start_container_monitoring(
    host: &DockerHost,
    truncated_id: &str,
    tx: &EventSender,
    active_containers: &mut HashMap<String, tokio::task::JoinHandle<()>>,
) {
    let tx_clone = tx.clone();
    let host_clone = host.clone();
    let truncated_id_clone = truncated_id.to_string();

    let handle = tokio::spawn(async move {
        stream_container_stats(host_clone, truncated_id_clone, tx_clone).await;
    });

    active_containers.insert(truncated_id.to_string(), handle);
}

/// Handles a container start event
async fn handle_container_start(
    host: &DockerHost,
    container_id: &str,
    tx: &EventSender,
    active_containers: &mut HashMap<String, tokio::task::JoinHandle<()>>,
) {
    let truncated_id = container_id[..12.min(container_id.len())].to_string();

    // Get container details
    if let Ok(inspect) = host
        .docker
        .inspect_container(container_id, None::<InspectContainerOptions>)
        .await
    {
        let name = inspect
            .name
            .as_ref()
            .map(|n| n.trim_start_matches('/').to_string())
            .unwrap_or_default();

        let status = inspect
            .state
            .as_ref()
            .and_then(|s| s.status.as_ref())
            .map(|s| format!("{:?}", s))
            .unwrap_or_else(|| "running".to_string());

        // Start monitoring the new container
        if !active_containers.contains_key(&truncated_id) {
            let container = Container {
                id: truncated_id.clone(),
                name: name.clone(),
                status: status.clone(),
                stats: ContainerStats::default(),
                host_id: host.host_id.clone(),
            };

            let _ = tx.send(AppEvent::ContainerCreated(container)).await;

            start_container_monitoring(host, &truncated_id, tx, active_containers);
        }
    }
}

/// Handles a container stop/die event
async fn handle_container_stop(
    host: &DockerHost,
    container_id: &str,
    tx: &EventSender,
    active_containers: &mut HashMap<String, tokio::task::JoinHandle<()>>,
) {
    let truncated_id = container_id[..12.min(container_id.len())].to_string();

    // Stop monitoring and notify removal
    if let Some(handle) = active_containers.remove(&truncated_id) {
        handle.abort();
        let key = ContainerKey::new(host.host_id.clone(), truncated_id);
        let _ = tx.send(AppEvent::ContainerDestroyed(key)).await;
    }
}
