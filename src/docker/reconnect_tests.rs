//! End-to-end tests for the reconnect state machine in
//! [`crate::docker::connection::container_manager`].
//!
//! These drive a real Bollard client against a stand-in daemon on a unix socket,
//! so the daemon can be killed and restarted mid-test the way a `docker` package
//! upgrade does. Unix sockets only; the test CI runs on Linux.
#![cfg(unix)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tokio::task::{JoinHandle, JoinSet};

use crate::core::types::AppEvent;
use crate::docker::connection::{DockerHost, connect_docker, container_manager};

/// How long a test waits for an expected event before giving up. Generous
/// because the reconnect backoff sleeps a second between ping attempts.
const EVENT_TIMEOUT: Duration = Duration::from_secs(20);

/// A minimal stand-in for `dockerd`: it serves only the endpoints the container
/// manager touches and can be stopped and restarted on the same socket path.
struct FakeDocker {
    path: PathBuf,
    acceptor: Option<JoinHandle<()>>,
    containers_json: &'static str,
}

impl FakeDocker {
    /// Binds the socket and starts serving `containers_json` as the container list.
    fn start(path: &Path, containers_json: &'static str) -> Self {
        let mut daemon = Self {
            path: path.to_path_buf(),
            acceptor: None,
            containers_json,
        };
        daemon.restart(containers_json);
        daemon
    }

    /// (Re)binds the socket, optionally serving a different container list — used
    /// to prove that a reconnect picks up changes made while the daemon was down.
    fn restart(&mut self, containers_json: &'static str) {
        self.stop();
        self.containers_json = containers_json;

        // A dropped UnixListener leaves its socket file behind, so clear it out
        // before rebinding at the same path.
        let _ = std::fs::remove_file(&self.path);
        let listener = UnixListener::bind(&self.path).expect("bind fake docker socket");

        self.acceptor = Some(tokio::spawn(async move {
            // Connections live in a JoinSet owned by this task: aborting the task
            // drops the set, which aborts every connection and closes its socket —
            // exactly what a daemon going away looks like to a client.
            let mut connections = JoinSet::new();

            while let Ok((stream, _)) = listener.accept().await {
                connections.spawn(handle_connection(stream, containers_json));
            }
        }));
    }

    /// Kills the daemon: stops accepting and drops every open connection.
    fn stop(&mut self) {
        if let Some(acceptor) = self.acceptor.take() {
            acceptor.abort();
        }
    }
}

impl Drop for FakeDocker {
    fn drop(&mut self) {
        self.stop();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Serves requests on one connection until the client goes away.
async fn handle_connection(mut stream: UnixStream, containers_json: &'static str) {
    let mut buffer = Vec::new();

    loop {
        // Read up to the end of the request head. Bollard only sends bodyless GETs
        // to the endpoints we serve, so the head is the whole request.
        let head_end = loop {
            if let Some(index) = find_head_end(&buffer) {
                break index;
            }

            let mut chunk = [0u8; 1024];
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return, // client closed
                Ok(n) => buffer.extend_from_slice(&chunk[..n]),
            }
        };

        let request = String::from_utf8_lossy(&buffer[..head_end]).to_string();
        buffer.drain(..head_end);

        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("")
            .to_string();

        if path.contains("/events") {
            // Headers only: the event stream stays open with no events until the
            // daemon is stopped, which is when the client sees the connection drop.
            let ok = stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n",
                )
                .await;
            if ok.is_err() {
                return;
            }
            let _ = stream.flush().await;

            // Park forever; the task is aborted when the daemon stops.
            std::future::pending::<()>().await;
            return;
        }

        let body = if path.contains("/_ping") {
            "OK".to_string()
        } else if path.contains("/containers/json") {
            containers_json.to_string()
        } else if path.contains("/containers/") && path.ends_with("/json") {
            // Container inspect — only `RestartCount` is read from it.
            "{\"RestartCount\":0}".to_string()
        } else {
            // Anything else (e.g. the stats stream) is not part of these tests.
            let _ = stream
                .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                .await;
            continue;
        };

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

        if stream.write_all(response.as_bytes()).await.is_err() {
            return;
        }
    }
}

/// Returns the index just past the blank line terminating an HTTP request head.
fn find_head_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

/// Waits for the first event matching `predicate`, failing the test on timeout.
async fn wait_for<T>(
    rx: &mut mpsc::Receiver<AppEvent>,
    what: &str,
    mut predicate: impl FnMut(&AppEvent) -> Option<T>,
) -> T {
    let deadline = tokio::time::Instant::now() + EVENT_TIMEOUT;

    loop {
        let event = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
            .unwrap_or_else(|| panic!("event channel closed while waiting for {what}"));

        if let Some(value) = predicate(&event) {
            return value;
        }
    }
}

/// Socket paths are scoped to the process: the module tree is compiled into more
/// than one test binary, and unix socket paths are global state.
fn socket_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("dtop-test-{}-{name}.sock", std::process::id()))
}

/// Leaves a socket file at `path` with nothing listening on it.
///
/// Bollard refuses to build a client for a socket that does not exist at all, so
/// "daemon is down" has to be modelled as a leftover socket file that refuses
/// connections — which is also what a stopped daemon leaves behind.
fn dead_socket(path: &Path) {
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path).expect("bind socket");
    drop(listener);
    assert!(path.exists(), "dropping the listener should leave the file");
}

fn docker_host(path: &Path) -> DockerHost {
    let docker = connect_docker(&format!("unix://{}", path.display())).expect("connect to socket");
    DockerHost::new("test".to_string(), docker, None, HashMap::new())
}

/// The whole point of the fix: a daemon that goes away and comes back should
/// leave the container manager running, reporting the outage and re-syncing.
#[tokio::test]
async fn container_manager_reconnects_after_daemon_restart() {
    const BEFORE: &str =
        r#"[{"Id":"aaaaaaaaaaaa1111","Names":["/web"],"State":"exited","Created":1700000000}]"#;
    // The daemon comes back with a different set of containers, so a resync is
    // distinguishable from stale state left over from before the outage.
    const AFTER: &str =
        r#"[{"Id":"bbbbbbbbbbbb2222","Names":["/db"],"State":"exited","Created":1700000000}]"#;

    let path = socket_path("reconnect");
    let mut daemon = FakeDocker::start(&path, BEFORE);

    let (tx, mut rx) = mpsc::channel::<AppEvent>(100);
    let manager = tokio::spawn(container_manager(docker_host(&path), tx));

    let names = wait_for(&mut rx, "the initial container list", |event| match event {
        AppEvent::InitialContainerList(_, list) => {
            Some(list.iter().map(|c| c.name.clone()).collect::<Vec<_>>())
        }
        _ => None,
    })
    .await;
    assert_eq!(names, vec!["web"]);

    // The daemon goes away, as during a package upgrade.
    daemon.stop();

    wait_for(&mut rx, "HostDisconnected", |event| match event {
        AppEvent::HostDisconnected(host_id) => Some(host_id.clone()),
        _ => None,
    })
    .await;

    // ...and comes back with a different container set.
    daemon.restart(AFTER);

    wait_for(&mut rx, "HostReconnected", |event| match event {
        AppEvent::HostReconnected(host_id) => Some(host_id.clone()),
        _ => None,
    })
    .await;

    let names = wait_for(
        &mut rx,
        "the re-synced container list",
        |event| match event {
            AppEvent::InitialContainerList(_, list) => {
                Some(list.iter().map(|c| c.name.clone()).collect::<Vec<_>>())
            }
            _ => None,
        },
    )
    .await;
    assert_eq!(
        names,
        vec!["db"],
        "reconnect should re-sync from the daemon, not replay pre-outage state"
    );

    manager.abort();
}

/// A host that is unreachable from the start must not spin: the manager should
/// report it as disconnected and keep retrying rather than exiting.
#[tokio::test]
async fn container_manager_reports_disconnect_when_host_never_answers() {
    let path = socket_path("never-answers");
    dead_socket(&path);

    let (tx, mut rx) = mpsc::channel::<AppEvent>(100);
    let manager = tokio::spawn(container_manager(docker_host(&path), tx));

    wait_for(&mut rx, "HostDisconnected", |event| match event {
        AppEvent::HostDisconnected(host_id) => Some(host_id.clone()),
        _ => None,
    })
    .await;

    // Still alive and retrying rather than having returned.
    assert!(!manager.is_finished());
    manager.abort();
}

/// The manager must stop once the UI is gone instead of retrying forever.
#[tokio::test]
async fn container_manager_stops_when_event_channel_closes() {
    let path = socket_path("channel-closed");
    dead_socket(&path);

    let (tx, rx) = mpsc::channel::<AppEvent>(100);
    let host = docker_host(&path);

    // Close the channel before starting, so the manager sees a gone UI on its
    // very first check rather than racing with it.
    drop(rx);

    let manager = tokio::spawn(container_manager(host, tx));

    tokio::time::timeout(EVENT_TIMEOUT, manager)
        .await
        .expect("container manager should exit once the channel closes")
        .expect("container manager panicked");
}
