use bollard::query_parameters::{
    RemoveContainerOptions, RestartContainerOptions, StartContainerOptions, StopContainerOptions,
};

use crate::core::types::{AppEvent, ContainerAction, ContainerKey, EventSender};
use crate::docker::connection::DockerHost;

/// Executes a container action asynchronously
pub async fn execute_container_action(
    host: DockerHost,
    container_key: ContainerKey,
    action: ContainerAction,
    tx: EventSender,
) {
    // Send in-progress event
    let _ = tx
        .send(AppEvent::ActionInProgress(container_key.clone(), action))
        .await;

    // Execute the action
    let result = match action {
        ContainerAction::Start => start_container(&host, &container_key.container_id).await,
        ContainerAction::Stop => stop_container(&host, &container_key.container_id).await,
        ContainerAction::Restart => restart_container(&host, &container_key.container_id).await,
        ContainerAction::Remove => remove_container(&host, &container_key.container_id).await,
    };

    // Send result event
    match result {
        Ok(_) => {
            let _ = tx
                .send(AppEvent::ActionSuccess(container_key, action))
                .await;
        }
        Err(err) => {
            let _ = tx
                .send(AppEvent::ActionError(container_key, action, err))
                .await;
        }
    }
}

/// Starts a container
async fn start_container(host: &DockerHost, container_id: &str) -> Result<(), String> {
    let options = StartContainerOptions { detach_keys: None };

    host.docker
        .start_container(container_id, Some(options))
        .await
        .map_err(|e| format!("Failed to start container: {}", e))
}

/// Stops a container with a 10-second timeout
async fn stop_container(host: &DockerHost, container_id: &str) -> Result<(), String> {
    let options = StopContainerOptions {
        signal: None,
        t: Some(10), // 10 second timeout before force kill
    };

    host.docker
        .stop_container(container_id, Some(options))
        .await
        .map_err(|e| format!("Failed to stop container: {}", e))
}

/// Restarts a container with a 10-second timeout
async fn restart_container(host: &DockerHost, container_id: &str) -> Result<(), String> {
    let options = RestartContainerOptions {
        signal: None,
        t: Some(10), // 10 second timeout before force kill
    };

    host.docker
        .restart_container(container_id, Some(options))
        .await
        .map_err(|e| format!("Failed to restart container: {}", e))
}

/// Removes a container (with force option if needed)
async fn remove_container(host: &DockerHost, container_id: &str) -> Result<(), String> {
    let options = RemoveContainerOptions {
        force: true, // Force removal even if running
        v: false,    // Don't remove volumes
        link: false,
    };

    host.docker
        .remove_container(container_id, Some(options))
        .await
        .map_err(|e| format!("Failed to remove container: {}", e))
}
