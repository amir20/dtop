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

    // Execute the action using DockerHost methods
    let result = match action {
        ContainerAction::Start => host.start_container(&container_key.container_id).await,
        ContainerAction::Stop => host.stop_container(&container_key.container_id).await,
        ContainerAction::Restart => host.restart_container(&container_key.container_id).await,
        ContainerAction::Remove => host.remove_container(&container_key.container_id).await,
        ContainerAction::Shell => {
            // Shell is handled separately in main.rs via StartShell event
            // This path should never be reached
            return;
        }
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
