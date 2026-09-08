//! Tests for the stats-stream retry in
//! [`crate::docker::stats::stream_container_stats`].
//!
//! A stats stream that breaks used to end its task for good, leaving the
//! container's row at zero for every metric for the rest of the session with
//! nothing logged. These drive a real Bollard client against a stand-in daemon
//! that breaks the stream on purpose.
#![cfg(unix)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

use crate::core::types::AppEvent;
use crate::docker::connection::{DockerHost, connect_docker};
use crate::docker::stats::stream_container_stats;

/// Generous enough to cover the one-second retry backoff between passes.
const EVENT_TIMEOUT: Duration = Duration::from_secs(20);

const CONTAINER_ID: &str = "abcdef123456";

/// One stats sample. `pids_stats.current` is the assertion handle: it is a raw
/// counter, so a value arriving intact proves a real sample was parsed rather
/// than a default-filled [`crate::core::types::ContainerStats`].
const STATS_SAMPLE: &str = r#"{"pids_stats":{"current":7,"limit":0},"cpu_stats":{"cpu_usage":{"total_usage":100},"system_cpu_usage":1000,"online_cpus":2},"precpu_stats":{"cpu_usage":{"total_usage":50},"system_cpu_usage":500,"online_cpus":2},"memory_stats":{"usage":1000,"limit":4000}}"#;

/// A stand-in daemon that answers the two endpoints the stats task touches:
/// the stats stream, and the inspect used to decide whether a retry is warranted.
///
/// Every stats stream it serves delivers exactly one sample and then drops the
/// connection mid-body, which is what a daemon hiccup looks like to the client.
struct FlakyStatsDaemon {
    path: PathBuf,
    acceptor: tokio::task::JoinHandle<()>,
    stats_requests: Arc<AtomicUsize>,
}

impl FlakyStatsDaemon {
    fn start(path: &Path) -> Self {
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path).expect("bind fake docker socket");
        let stats_requests = Arc::new(AtomicUsize::new(0));

        let counter = Arc::clone(&stats_requests);
        let acceptor = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let counter = Arc::clone(&counter);
                tokio::spawn(handle_connection(stream, counter));
            }
        });

        Self {
            path: path.to_path_buf(),
            acceptor,
            stats_requests,
        }
    }

    /// How many times the client has opened the stats stream.
    fn stats_requests(&self) -> usize {
        self.stats_requests.load(Ordering::SeqCst)
    }
}

impl Drop for FlakyStatsDaemon {
    fn drop(&mut self) {
        self.acceptor.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}

async fn handle_connection(mut stream: UnixStream, stats_requests: Arc<AtomicUsize>) {
    let mut buffer = Vec::new();

    loop {
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

        if path.contains("/stats") {
            stats_requests.fetch_add(1, Ordering::SeqCst);

            // One sample, then hang up mid-body: the client sees a transport
            // error rather than a clean end of stream.
            let head = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n";
            if stream.write_all(head).await.is_err() {
                return;
            }
            let chunk = format!("{:x}\r\n{}\r\n", STATS_SAMPLE.len(), STATS_SAMPLE);
            let _ = stream.write_all(chunk.as_bytes()).await;
            let _ = stream.flush().await;
            return;
        }

        // Container inspect: the retry only continues while the container runs.
        let body = if path.contains("/containers/") && path.ends_with("/json") {
            "{\"State\":{\"Running\":true}}".to_string()
        } else {
            "OK".to_string()
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

fn find_head_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

/// Socket paths are process-global, and this module is compiled into more than
/// one test binary.
fn socket_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dtop-stats-test-{}-{name}.sock",
        std::process::id()
    ))
}

async fn next_stat(rx: &mut mpsc::Receiver<AppEvent>) -> u64 {
    let deadline = tokio::time::Instant::now() + EVENT_TIMEOUT;

    loop {
        let event = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .expect("timed out waiting for a stats update")
            .expect("event channel closed while waiting for a stats update");

        if let AppEvent::ContainerStat(key, stats) = event {
            assert_eq!(key.container_id, CONTAINER_ID);
            return stats.pids_current;
        }
    }
}

/// A broken stats stream is re-opened while the container is still running.
///
/// Without the retry the task ended after the first sample and the row stayed at
/// zero forever: nothing re-armed monitoring short of a fresh `start` event,
/// which a container that never stopped does not emit.
#[tokio::test]
async fn test_stats_stream_reconnects_after_the_daemon_breaks_it() {
    let path = socket_path("retry");
    let daemon = FlakyStatsDaemon::start(&path);

    let docker = connect_docker(&format!("unix://{}", path.display())).expect("connect");
    // Not "local": that host id enables the cgroups v2 filesystem fallback,
    // which has nothing to do with what is under test here.
    let host = DockerHost::new("fake".to_string(), docker, None, HashMap::new());

    let (tx, mut rx) = mpsc::channel(64);
    let task = tokio::spawn(stream_container_stats(
        host,
        CONTAINER_ID.to_string(),
        tx.clone(),
    ));

    // Each pass yields exactly one sample before the daemon hangs up, so a second
    // sample can only come from a re-opened stream.
    assert_eq!(next_stat(&mut rx).await, 7, "first sample");
    assert_eq!(next_stat(&mut rx).await, 7, "sample after the stream broke");
    assert!(
        daemon.stats_requests() >= 2,
        "expected the stats stream to be re-opened, saw {} request(s)",
        daemon.stats_requests()
    );

    task.abort();
}

/// Dropping the receiver stops the retry loop, so quitting does not leave tasks
/// reconnecting to a UI that is gone.
#[tokio::test]
async fn test_stats_stream_stops_retrying_when_the_ui_goes_away() {
    let path = socket_path("closed-channel");
    let daemon = FlakyStatsDaemon::start(&path);

    let docker = connect_docker(&format!("unix://{}", path.display())).expect("connect");
    let host = DockerHost::new("fake".to_string(), docker, None, HashMap::new());

    let (tx, mut rx) = mpsc::channel(64);
    let task = tokio::spawn(stream_container_stats(
        host,
        CONTAINER_ID.to_string(),
        tx.clone(),
    ));

    assert_eq!(next_stat(&mut rx).await, 7);
    drop(rx);
    drop(tx);

    tokio::time::timeout(EVENT_TIMEOUT, task)
        .await
        .expect("stats task kept running after the channel closed")
        .expect("stats task panicked");

    // It gave up rather than hammering the daemon.
    assert!(daemon.stats_requests() <= 2);
}
