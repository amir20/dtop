//! Tests for log history and streaming against a stand-in daemon that filters
//! by time the way `dockerd` does.
//!
//! The Docker logs API takes `since`/`until` in whole seconds and compares them
//! against nanosecond log timestamps: `since` keeps `ts >= since.000` and
//! `until` keeps `ts <= until.000`. Both functions under test have to widen
//! their windows to whole seconds and trim to exact timestamps themselves.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::core::types::AppEvent;
use crate::docker::connection::{DockerHost, connect_docker};
use crate::docker::logs::{fetch_older_logs, stream_container_logs};

const TIMEOUT: Duration = Duration::from_secs(10);

/// The second every test log line is written in.
const SECOND: i64 = 1_700_000_000;

/// Line `n` (1-based), all within [`SECOND`], `n` nanoseconds in.
fn ts(n: u32) -> DateTime<Utc> {
    Utc.timestamp_opt(SECOND, n).unwrap()
}

fn line(n: u32) -> String {
    format!(
        "{} line {n}\n",
        ts(n).to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
    )
}

/// Serves `/containers/{id}/logs` from `history`, plus `live` appended to a
/// `follow=true` response, applying dockerd's `since`/`until`/`tail` rules.
struct FakeLogs {
    path: PathBuf,
    task: JoinHandle<()>,
}

impl FakeLogs {
    fn start(name: &str, history: Vec<u32>, live: Vec<u32>) -> Self {
        let path =
            std::env::temp_dir().join(format!("dtop-test-{}-{name}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).expect("bind fake docker socket");

        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                tokio::spawn(serve(stream, history.clone(), live.clone()));
            }
        });

        Self { path, task }
    }

    fn host(&self) -> DockerHost {
        let docker = connect_docker(&format!("unix://{}", self.path.display())).expect("connect");
        DockerHost::new("test".to_string(), docker, None, HashMap::new())
    }
}

impl Drop for FakeLogs {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}

async fn serve(mut stream: UnixStream, history: Vec<u32>, live: Vec<u32>) {
    let mut buffer = Vec::new();
    loop {
        let head_end = loop {
            if let Some(i) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
            let mut chunk = [0u8; 1024];
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(n) => buffer.extend_from_slice(&chunk[..n]),
            }
        };
        let request = String::from_utf8_lossy(&buffer[..head_end]).to_string();
        buffer.drain(..head_end);

        let target = request.split_whitespace().nth(1).unwrap_or("");
        let query: HashMap<&str, &str> = target
            .split_once('?')
            .map(|(_, q)| q.split('&').filter_map(|kv| kv.split_once('=')).collect())
            .unwrap_or_default();
        let number = |key: &str| {
            query
                .get(key)
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0)
        };
        let (since, until) = (number("since"), number("until"));
        let follow = query.get("follow") == Some(&"true");

        let in_window = |n: &u32| {
            let t = ts(*n);
            (since == 0 || t >= Utc.timestamp_opt(since, 0).unwrap())
                && (until == 0 || t <= Utc.timestamp_opt(until, 0).unwrap())
        };
        let mut lines: Vec<u32> = history.iter().copied().filter(in_window).collect();
        if let Some(tail) = query.get("tail").and_then(|v| v.parse::<usize>().ok()) {
            lines = lines.split_off(lines.len().saturating_sub(tail));
        }
        if follow {
            lines.extend(live.iter().copied().filter(in_window));
        }
        let body: String = lines.into_iter().map(line).collect();

        if follow {
            let head =
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\n\r\n";
            let chunk = format!("{:x}\r\n{body}\r\n", body.len());
            if stream.write_all(head.as_bytes()).await.is_err()
                || (!body.is_empty() && stream.write_all(chunk.as_bytes()).await.is_err())
            {
                return;
            }
            // Keep following until the client goes away
            std::future::pending::<()>().await;
        }

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        if stream.write_all(response.as_bytes()).await.is_err() {
            return;
        }
    }
}

/// Returns the line numbers in a batch of log entries.
fn numbers(entries: &[crate::docker::logs::LogEntry]) -> Vec<u32> {
    entries
        .iter()
        .map(|e| e.timestamp.timestamp_subsec_nanos())
        .collect()
}

/// The live stream restarts at the last history line's whole second, so it
/// replays that second; nothing already shown may be sent again.
#[tokio::test]
async fn streaming_does_not_repeat_the_history_boundary() {
    let fake = FakeLogs::start("logs-boundary", (1..=5).collect(), vec![6, 7]);
    let (tx, mut rx) = mpsc::channel(100);
    let task = tokio::spawn(stream_container_logs(fake.host(), "abc".to_string(), tx));

    let mut history = Vec::new();
    let mut live = Vec::new();
    while live.len() < 2 {
        match tokio::time::timeout(TIMEOUT, rx.recv()).await {
            Ok(Some(AppEvent::LogBatchPrepend(_, batch, _))) => history = numbers(&batch),
            Ok(Some(AppEvent::LogLine(_, entry))) => live.push(numbers(&[entry])[0]),
            Ok(Some(_)) => {}
            other => panic!("stream ended early: {other:?}"),
        }
    }

    assert_eq!(history, [1, 2, 3, 4, 5]);
    assert_eq!(
        live,
        [6, 7],
        "history lines were replayed into the live stream"
    );
    task.abort();
}

/// Older history in the same second as the oldest loaded line must still be
/// reachable; a burst of logs can put an entire page in one second.
#[tokio::test]
async fn older_logs_include_the_rest_of_the_boundary_second() {
    let fake = FakeLogs::start("logs-older", (1..=5).collect(), vec![]);
    let (tx, mut rx) = mpsc::channel(100);

    // Lines 4 and 5 are already loaded
    fetch_older_logs(
        fake.host(),
        "abc".to_string(),
        ts(4),
        ts(5),
        Some(Utc.timestamp_opt(SECOND - 60, 0).unwrap()),
        1000,
        tx,
    )
    .await;

    let Ok(Some(AppEvent::LogBatchPrepend(_, batch, has_more))) =
        tokio::time::timeout(TIMEOUT, rx.recv()).await
    else {
        panic!("no batch sent");
    };
    assert_eq!(numbers(&batch), [1, 2, 3]);
    assert!(!has_more, "the window reached the container's start");
}
