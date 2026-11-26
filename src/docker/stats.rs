use bollard::models::ContainerStatsResponse;
use bollard::query_parameters::StatsOptions;
use futures_util::stream::StreamExt;
use std::time::{Duration, Instant};

use crate::core::types::{AppEvent, ContainerKey, ContainerStats, EventSender};
use crate::docker::connection::DockerHost;

/// Smoothing factor for the exponential moving average applied to stats.
/// Higher alpha = more responsive, lower alpha = smoother. 0.3 balances the two.
const ALPHA: f64 = 0.3;

/// Delay before the first attempt to re-open a container's stats stream.
const INITIAL_STATS_RETRY_DELAY: Duration = Duration::from_secs(1);
/// Upper bound on the stats retry backoff.
const MAX_STATS_RETRY_DELAY: Duration = Duration::from_secs(5);

/// How long [`sample_container_stats`] waits for the frames it needs.
const SAMPLE_TIMEOUT: Duration = Duration::from_secs(5);

/// Exponentially smoothed values carried across samples.
///
/// These survive a reconnect: the container kept running, so its previous
/// smoothed values are still the best estimate to blend the next sample into.
struct SmoothedStats {
    /// Weight of each new sample; 1.0 turns smoothing off
    alpha: f64,
    cpu: Option<f64>,
    memory: Option<f64>,
    net_tx: Option<f64>,
    net_rx: Option<f64>,
    disk_read: Option<f64>,
    disk_write: Option<f64>,
}

impl Default for SmoothedStats {
    fn default() -> Self {
        Self {
            alpha: ALPHA,
            cpu: None,
            memory: None,
            net_tx: None,
            net_rx: None,
            disk_read: None,
            disk_write: None,
        }
    }
}

/// Why a single pass over the stats stream stopped.
enum StreamOutcome {
    /// The daemon closed the stream, or it errored. The container may well still
    /// be running, so this is worth retrying.
    Interrupted,
    /// The UI is gone; nothing left to send stats to.
    ChannelClosed,
}

/// Streams stats for a single container and sends updates via the event channel
///
/// Uses exponential decay smoothing to reduce noise in stats:
/// smoothed = alpha * new_value + (1 - alpha) * previous_smoothed
///
/// The stream is re-opened with a capped backoff whenever it breaks while the
/// container is still running. Without that, a single hiccup left the container
/// stuck at zero for every metric for the rest of the session: nothing re-armed
/// monitoring except a fresh `start` event, which a container that never stopped
/// does not emit.
///
/// # Arguments
/// * `host` - Docker host instance with identifier
/// * `truncated_id` - Truncated container ID (12 chars) - Docker API accepts partial IDs
/// * `tx` - Event sender channel
pub async fn stream_container_stats(host: DockerHost, truncated_id: String, tx: EventSender) {
    let mut smoothed = SmoothedStats::default();
    let mut cached_cgroup_path: CgroupPathCache = None;
    let mut delay = INITIAL_STATS_RETRY_DELAY;

    loop {
        let outcome = stream_stats_once(
            &host,
            &truncated_id,
            &tx,
            &mut smoothed,
            &mut cached_cgroup_path,
        )
        .await;

        if matches!(outcome, StreamOutcome::ChannelClosed) || tx.is_closed() {
            return;
        }

        // Retrying a container that has stopped or been removed would spin
        // against a 404 forever; the lifecycle events own that transition.
        if !is_container_running(&host, &truncated_id).await {
            tracing::debug!(
                "Stats stream ended for container {} on host {} (no longer running)",
                truncated_id,
                host.host_id
            );
            return;
        }

        tracing::warn!(
            "Stats stream for container {} on host {} ended while it is still running; retrying in {:?}",
            truncated_id,
            host.host_id,
            delay
        );

        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(MAX_STATS_RETRY_DELAY);
    }
}

/// Takes one unsmoothed reading of a container's stats, for a one-off snapshot.
///
/// A stream's first frame has no previous sample, so its CPU and rates read
/// zero; the second, about a second later, is the first real reading. If the
/// second doesn't arrive in time, the first is returned (memory and PIDs are
/// still right in it).
pub async fn sample_container_stats(
    host: &DockerHost,
    truncated_id: &str,
) -> Option<ContainerStats> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(2);
    let mut smoothed = SmoothedStats {
        alpha: 1.0,
        ..Default::default()
    };
    let mut cached_cgroup_path = None;
    let stream = stream_stats_once(
        host,
        truncated_id,
        &tx,
        &mut smoothed,
        &mut cached_cgroup_path,
    );
    tokio::pin!(stream);
    let deadline = tokio::time::sleep(SAMPLE_TIMEOUT);
    tokio::pin!(deadline);

    let mut latest = None;
    for _ in 0..2 {
        tokio::select! {
            Some(AppEvent::ContainerStat(_, stats)) = rx.recv() => latest = Some(stats),
            _ = &mut stream => {
                // The stream ended; keep whatever it sent before it did
                while let Ok(AppEvent::ContainerStat(_, stats)) = rx.try_recv() {
                    latest = Some(stats);
                }
                break;
            }
            _ = &mut deadline => break,
        }
    }
    latest
}

/// Opens the stats stream once and pumps it until it ends.
///
/// Rate calculations need two consecutive samples, so the previous counters live
/// here rather than across reconnects: the gap over an outage is not a rate we
/// can report honestly.
async fn stream_stats_once(
    host: &DockerHost,
    truncated_id: &str,
    tx: &EventSender,
    smoothed: &mut SmoothedStats,
    cached_cgroup_path: &mut CgroupPathCache,
) -> StreamOutcome {
    let stats_options = StatsOptions {
        stream: true,
        one_shot: false,
    };

    let mut stats_stream = host.docker.stats(truncated_id, Some(stats_options));

    // Track previous network stats for rate calculation
    let mut prev_net_tx: Option<u64> = None;
    let mut prev_net_rx: Option<u64> = None;
    let mut prev_timestamp: Option<Instant> = None;

    // Track previous disk I/O stats for rate calculation
    let mut prev_disk_read: Option<u64> = None;
    let mut prev_disk_write: Option<u64> = None;

    // Check if host is local before permitting local cgroups v2 filesystem fallbacks
    let is_local_host = host.is_local;

    while let Some(result) = stats_stream.next().await {
        match result {
            Ok(stats) => {
                let cpu_percent = calculate_cpu_percentage(&stats);
                let memory_percent = calculate_memory_percentage(&stats);
                let (net_tx_rate, net_rx_rate) =
                    calculate_network_rates(&stats, prev_net_tx, prev_net_rx, prev_timestamp);

                // Extract cumulative disk bytes exactly once per tick. The Docker API
                // path is cheap; only the cgroups v2 filesystem fallback (moby#35352)
                // touches the disk, and that runs off the async reactor via spawn_blocking.
                let (disk_read_bytes, disk_write_bytes) = {
                    let (read, write) = extract_disk_bytes(&stats);
                    if read.is_some() || write.is_some() {
                        (read, write)
                    } else if is_local_host {
                        let id = truncated_id.to_string();
                        let path = cached_cgroup_path.take();
                        match tokio::task::spawn_blocking(move || {
                            let mut path = path;
                            let res = extract_cgroup_v2_disk_bytes(
                                std::path::Path::new(CGROUP_ROOT),
                                &id,
                                &mut path,
                            );
                            (res, path)
                        })
                        .await
                        {
                            Ok((res, path)) => {
                                *cached_cgroup_path = path;
                                res
                            }
                            Err(_) => (None, None),
                        }
                    } else {
                        (None, None)
                    }
                };

                let (disk_read_rate, disk_write_rate) = calculate_disk_rates(
                    disk_read_bytes,
                    disk_write_bytes,
                    prev_disk_read,
                    prev_disk_write,
                    prev_timestamp,
                );

                // Update previous network values for next iteration
                let (tx_bytes, rx_bytes) = extract_network_bytes(&stats);
                prev_net_tx = tx_bytes;
                prev_net_rx = rx_bytes;

                // Update previous disk I/O values for next iteration
                prev_disk_read = disk_read_bytes;
                prev_disk_write = disk_write_bytes;

                prev_timestamp = Some(Instant::now());

                // Apply exponential moving average (first value passes through unsmoothed)
                let cpu = ema(smoothed.cpu, cpu_percent, smoothed.alpha);
                let memory = ema(smoothed.memory, memory_percent, smoothed.alpha);
                let network_tx_bytes_per_sec = ema(smoothed.net_tx, net_tx_rate, smoothed.alpha);
                let network_rx_bytes_per_sec = ema(smoothed.net_rx, net_rx_rate, smoothed.alpha);
                let disk_read_bytes_per_sec =
                    ema(smoothed.disk_read, disk_read_rate, smoothed.alpha);
                let disk_write_bytes_per_sec =
                    ema(smoothed.disk_write, disk_write_rate, smoothed.alpha);

                // Update smoothed values for next iteration
                smoothed.cpu = Some(cpu);
                smoothed.memory = Some(memory);
                smoothed.net_tx = Some(network_tx_bytes_per_sec);
                smoothed.net_rx = Some(network_rx_bytes_per_sec);
                smoothed.disk_read = Some(disk_read_bytes_per_sec);
                smoothed.disk_write = Some(disk_write_bytes_per_sec);

                // Extract raw memory bytes for display
                let (memory_used_bytes, memory_limit_bytes) = extract_memory_bytes(&stats);

                // Extract PID counts (raw counts, no smoothing needed)
                let (pids_current, pids_limit) = extract_pids(&stats);

                let stats = ContainerStats {
                    cpu,
                    memory,
                    memory_used_bytes,
                    memory_limit_bytes,
                    network_tx_bytes_per_sec,
                    network_rx_bytes_per_sec,
                    disk_read_bytes_per_sec,
                    disk_write_bytes_per_sec,
                    pids_current,
                    pids_limit,
                };

                let key = ContainerKey::new(host.host_id.clone(), truncated_id.to_string());
                if tx.send(AppEvent::ContainerStat(key, stats)).await.is_err() {
                    return StreamOutcome::ChannelClosed;
                }
            }
            Err(e) => {
                // Swallowing this used to make the container's row sit at zero
                // with nothing to point at; the retry above needs it named.
                tracing::warn!(
                    "Stats stream error for container {} on host {}: {}",
                    truncated_id,
                    host.host_id,
                    e
                );
                return StreamOutcome::Interrupted;
            }
        }
    }

    StreamOutcome::Interrupted
}

/// Reports whether the container is still running, so a broken stats stream is
/// only retried while there is something left to measure.
///
/// A container that is gone (404) or stopped answers `false`; so does a daemon
/// we cannot reach, since the host-level reconnect in
/// [`crate::docker::connection::container_manager`] re-lists and re-arms
/// monitoring for the whole host once it comes back.
async fn is_container_running(host: &DockerHost, truncated_id: &str) -> bool {
    host.docker
        .inspect_container(
            truncated_id,
            None::<bollard::query_parameters::InspectContainerOptions>,
        )
        .await
        .ok()
        .and_then(|inspect| inspect.state.and_then(|state| state.running))
        .unwrap_or(false)
}

/// Applies one step of an exponential moving average.
///
/// Returns `sample` unchanged for the first value (when `prev` is `None`),
/// otherwise `alpha * sample + (1 - alpha) * prev`.
fn ema(prev: Option<f64>, sample: f64, alpha: f64) -> f64 {
    prev.map_or(sample, |prev| alpha * sample + (1.0 - alpha) * prev)
}

/// Calculates CPU usage percentage from container stats
pub fn calculate_cpu_percentage(stats: &ContainerStatsResponse) -> f64 {
    let Some(cpu_stats) = &stats.cpu_stats else {
        return 0.0;
    };
    let Some(precpu_stats) = &stats.precpu_stats else {
        return 0.0;
    };

    let cpu_usage = cpu_stats
        .cpu_usage
        .as_ref()
        .and_then(|u| u.total_usage)
        .unwrap_or(0);
    let precpu_usage = precpu_stats
        .cpu_usage
        .as_ref()
        .and_then(|u| u.total_usage)
        .unwrap_or(0);
    let cpu_delta = cpu_usage as f64 - precpu_usage as f64;

    let system_delta = cpu_stats.system_cpu_usage.unwrap_or(0) as f64
        - precpu_stats.system_cpu_usage.unwrap_or(0) as f64;
    let number_cpus = cpu_stats.online_cpus.unwrap_or(1) as f64;

    if system_delta > 0.0 && cpu_delta > 0.0 {
        (cpu_delta / system_delta) * number_cpus * 100.0
    } else {
        0.0
    }
}

/// Calculates memory usage percentage from container stats
///
/// Subtracts cache from raw usage to match `docker stats` behavior:
/// - cgroups v2: subtract `inactive_file`
/// - cgroups v1: subtract `cache`
pub fn calculate_memory_percentage(stats: &ContainerStatsResponse) -> f64 {
    let Some(memory_stats) = &stats.memory_stats else {
        return 0.0;
    };

    let memory_usage = calculate_used_memory(memory_stats);
    let memory_limit = memory_stats.limit.unwrap_or(0) as f64;

    if memory_limit > 0.0 {
        (memory_usage / memory_limit) * 100.0
    } else {
        0.0
    }
}

/// Extracts memory bytes (used, limit) from container stats
/// Subtracts cache to match `docker stats` behavior
fn extract_memory_bytes(stats: &ContainerStatsResponse) -> (u64, u64) {
    let Some(memory_stats) = &stats.memory_stats else {
        return (0, 0);
    };

    let memory_used = calculate_used_memory(memory_stats) as u64;
    let memory_limit = memory_stats.limit.unwrap_or(0);

    (memory_used, memory_limit)
}

/// Calculates used memory by subtracting cache from raw usage,
/// matching `docker stats` behavior.
///
/// On cgroups v2, subtracts `inactive_file`. On cgroups v1, subtracts `cache`.
/// Falls back to raw usage if neither is available.
fn calculate_used_memory(memory_stats: &bollard::models::ContainerMemoryStats) -> f64 {
    let usage = memory_stats.usage.unwrap_or(0) as f64;

    let cache = memory_stats
        .stats
        .as_ref()
        .and_then(|s| {
            // cgroups v2 uses inactive_file, cgroups v1 uses cache
            s.get("inactive_file").or_else(|| s.get("cache")).copied()
        })
        .unwrap_or(0) as f64;

    (usage - cache).max(0.0)
}

/// Extracts PID counts (current, limit) from container stats.
///
/// `pids_stats` is Linux-specific and absent for Windows containers, so both
/// values default to 0 when unavailable. A limit of 0 means "no limit".
fn extract_pids(stats: &ContainerStatsResponse) -> (u64, u64) {
    match &stats.pids_stats {
        Some(pids) => (pids.current.unwrap_or(0), pids.limit.unwrap_or(0)),
        None => (0, 0),
    }
}

/// Extracts total network bytes (tx, rx) from container stats
fn extract_network_bytes(stats: &ContainerStatsResponse) -> (Option<u64>, Option<u64>) {
    let Some(networks) = &stats.networks else {
        return (None, None);
    };

    let mut total_tx = 0u64;
    let mut total_rx = 0u64;

    for interface_stats in networks.values() {
        total_tx += interface_stats.tx_bytes.unwrap_or(0);
        total_rx += interface_stats.rx_bytes.unwrap_or(0);
    }

    (Some(total_tx), Some(total_rx))
}

/// Calculates network transfer rates in bytes per second
fn calculate_network_rates(
    stats: &ContainerStatsResponse,
    prev_tx: Option<u64>,
    prev_rx: Option<u64>,
    prev_time: Option<Instant>,
) -> (f64, f64) {
    let (current_tx, current_rx) = extract_network_bytes(stats);

    // If we don't have previous values, return 0
    let (Some(prev_tx), Some(prev_rx), Some(prev_time)) = (prev_tx, prev_rx, prev_time) else {
        return (0.0, 0.0);
    };

    let (Some(current_tx), Some(current_rx)) = (current_tx, current_rx) else {
        return (0.0, 0.0);
    };

    let elapsed = prev_time.elapsed().as_secs_f64();
    if elapsed <= 0.0 {
        return (0.0, 0.0);
    }

    let tx_delta = current_tx.saturating_sub(prev_tx) as f64;
    let rx_delta = current_rx.saturating_sub(prev_rx) as f64;

    let tx_rate = tx_delta / elapsed;
    let rx_rate = rx_delta / elapsed;

    (tx_rate, rx_rate)
}

/// Extracts total disk bytes (read, write) from container stats
///
/// Uses blkio_stats.io_service_bytes_recursive which contains cumulative bytes
/// for each operation type ("Read", "Write", etc.) across all devices.
fn extract_disk_bytes(stats: &ContainerStatsResponse) -> (Option<u64>, Option<u64>) {
    let Some(blkio_stats) = &stats.blkio_stats else {
        return (None, None);
    };

    // Only io_service_bytes_recursive reports bytes. io_serviced_recursive counts
    // operations (IOPS), not bytes, so it must not be used as a fallback here.
    let Some(entries) = blkio_stats.io_service_bytes_recursive.as_ref() else {
        return (None, None);
    };

    let mut total_read = 0u64;
    let mut total_write = 0u64;

    for entry in entries {
        let value = entry.value.unwrap_or(0);
        if let Some(op) = entry.op.as_deref() {
            if op.eq_ignore_ascii_case("read") {
                total_read += value;
            } else if op.eq_ignore_ascii_case("write") {
                total_write += value;
            }
        }
    }

    (Some(total_read), Some(total_write))
}

/// Helper to parse rbytes and wbytes from cgroups v2 io.stat file content
fn parse_io_stat_content(content: &str) -> (Option<u64>, Option<u64>) {
    let mut total_read = 0u64;
    let mut total_write = 0u64;
    let mut found = false;

    for line in content.lines() {
        for part in line.split_whitespace() {
            if let Some(r) = part.strip_prefix("rbytes=")
                && let Ok(val) = r.parse::<u64>()
            {
                total_read = total_read.saturating_add(val);
                found = true;
            } else if let Some(w) = part.strip_prefix("wbytes=")
                && let Ok(val) = w.parse::<u64>()
            {
                total_write = total_write.saturating_add(val);
                found = true;
            }
        }
    }

    if found {
        (Some(total_read), Some(total_write))
    } else {
        (None, None)
    }
}

/// Optimized fallback function to extract disk bytes from cgroups v2 io.stat file
/// when Docker Stats API returns null blkio_stats (moby/moby#35352).
///
/// Resolves the container's io.stat once and caches it for the stream. A miss is
/// cached too (`Some(None)`), so a host without these cgroups isn't re-scanned
/// every frame.
fn extract_cgroup_v2_disk_bytes(
    cgroup_root: &std::path::Path,
    container_id: &str,
    cached_path: &mut CgroupPathCache,
) -> (Option<u64>, Option<u64>) {
    if let Some(Some(path)) = cached_path
        && let Ok(content) = std::fs::read_to_string(path)
    {
        return parse_io_stat_content(&content);
    }
    if matches!(cached_path, Some(None)) {
        return (None, None);
    }

    let found = find_cgroup_io_stat(cgroup_root, container_id);
    let res = found
        .as_ref()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map_or((None, None), |content| parse_io_stat_content(&content));
    *cached_path = Some(found);
    res
}

/// `None` until looked up; then the io.stat path, or `None` when there is none.
type CgroupPathCache = Option<Option<std::path::PathBuf>>;

const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Finds the container's io.stat under the cgroupfs (`docker/<id>`) or systemd
/// (`system.slice/docker-<id>.scope`) layout. Both name the full 64-char ID, so
/// the truncated `container_id` is matched as a prefix.
fn find_cgroup_io_stat(root: &std::path::Path, container_id: &str) -> Option<std::path::PathBuf> {
    let entries = |dir: &str| {
        std::fs::read_dir(root.join(dir))
            .into_iter()
            .flatten()
            .flatten()
    };
    let cgroupfs = entries("docker").filter(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .starts_with(container_id)
    });
    let systemd = entries("system.slice").filter(|entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        name.strip_prefix("docker-")
            .is_some_and(|scope_id| scope_id.starts_with(container_id))
            && name.ends_with(".scope")
    });
    cgroupfs
        .chain(systemd)
        .map(|entry| entry.path().join("io.stat"))
        .find(|path| path.is_file())
}

/// Calculates disk I/O rates in bytes per second from pre-extracted cumulative
/// byte counters. Byte extraction (including the potentially-blocking cgroups v2
/// filesystem fallback) is done by the caller so it happens exactly once per tick.
fn calculate_disk_rates(
    current_read: Option<u64>,
    current_write: Option<u64>,
    prev_read: Option<u64>,
    prev_write: Option<u64>,
    prev_time: Option<Instant>,
) -> (f64, f64) {
    // If we don't have previous values, return 0
    let (Some(prev_read), Some(prev_write), Some(prev_time)) = (prev_read, prev_write, prev_time)
    else {
        return (0.0, 0.0);
    };

    let (Some(current_read), Some(current_write)) = (current_read, current_write) else {
        return (0.0, 0.0);
    };

    let elapsed = prev_time.elapsed().as_secs_f64();
    if elapsed <= 0.0 {
        return (0.0, 0.0);
    }

    let read_delta = current_read.saturating_sub(prev_read) as f64;
    let write_delta = current_write.saturating_sub(prev_write) as f64;

    (read_delta / elapsed, write_delta / elapsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::models::{
        ContainerBlkioStatEntry, ContainerBlkioStats, ContainerCpuStats, ContainerCpuUsage,
        ContainerMemoryStats,
    };

    fn create_cpu_stats(
        total_usage: u64,
        system_cpu_usage: u64,
        online_cpus: u32,
    ) -> ContainerCpuStats {
        ContainerCpuStats {
            cpu_usage: Some(ContainerCpuUsage {
                total_usage: Some(total_usage),
                percpu_usage: None,
                usage_in_kernelmode: None,
                usage_in_usermode: None,
            }),
            system_cpu_usage: Some(system_cpu_usage),
            online_cpus: Some(online_cpus),
            throttling_data: None,
        }
    }

    #[test]
    fn test_calculate_cpu_percentage_normal_usage() {
        let stats = ContainerStatsResponse {
            cpu_stats: Some(create_cpu_stats(1_000_000_000, 2_000_000_000, 4)),
            precpu_stats: Some(create_cpu_stats(500_000_000, 1_000_000_000, 4)),
            ..Default::default()
        };

        let cpu = calculate_cpu_percentage(&stats);

        // CPU delta: 1B - 500M = 500M
        // System delta: 2B - 1B = 1B
        // (500M / 1B) * 4 CPUs * 100 = 200%
        assert_eq!(cpu, 200.0);
    }

    #[test]
    fn test_calculate_cpu_percentage_single_core() {
        let stats = ContainerStatsResponse {
            cpu_stats: Some(create_cpu_stats(800_000_000, 1_000_000_000, 1)),
            precpu_stats: Some(create_cpu_stats(200_000_000, 500_000_000, 1)),
            ..Default::default()
        };

        let cpu = calculate_cpu_percentage(&stats);

        // CPU delta: 800M - 200M = 600M
        // System delta: 1B - 500M = 500M
        // (600M / 500M) * 1 CPU * 100 = 120%
        assert_eq!(cpu, 120.0);
    }

    #[test]
    fn test_calculate_cpu_percentage_missing_cpu_stats() {
        let stats = ContainerStatsResponse {
            cpu_stats: None,
            precpu_stats: None,
            ..Default::default()
        };

        assert_eq!(calculate_cpu_percentage(&stats), 0.0);
    }

    #[test]
    fn test_calculate_cpu_percentage_missing_precpu_stats() {
        let stats = ContainerStatsResponse {
            cpu_stats: Some(create_cpu_stats(1_000_000_000, 2_000_000_000, 4)),
            precpu_stats: None,
            ..Default::default()
        };

        assert_eq!(calculate_cpu_percentage(&stats), 0.0);
    }

    #[test]
    fn test_calculate_cpu_percentage_zero_system_delta() {
        let stats = ContainerStatsResponse {
            cpu_stats: Some(create_cpu_stats(1_000_000_000, 2_000_000_000, 4)),
            precpu_stats: Some(create_cpu_stats(500_000_000, 2_000_000_000, 4)), // Same system CPU
            ..Default::default()
        };

        // Should return 0.0 when system delta is 0
        assert_eq!(calculate_cpu_percentage(&stats), 0.0);
    }

    #[test]
    fn test_calculate_cpu_percentage_zero_cpu_delta() {
        let stats = ContainerStatsResponse {
            cpu_stats: Some(create_cpu_stats(1_000_000_000, 2_000_000_000, 4)),
            precpu_stats: Some(create_cpu_stats(1_000_000_000, 1_000_000_000, 4)), // Same CPU usage
            ..Default::default()
        };

        // Should return 0.0 when CPU delta is 0
        assert_eq!(calculate_cpu_percentage(&stats), 0.0);
    }

    #[test]
    fn test_calculate_memory_percentage_normal_usage() {
        let stats = ContainerStatsResponse {
            memory_stats: Some(ContainerMemoryStats {
                usage: Some(500_000_000),   // 500 MB
                limit: Some(1_000_000_000), // 1 GB
                max_usage: None,
                stats: None,
                failcnt: None,
                commitbytes: None,
                commitpeakbytes: None,
                privateworkingset: None,
            }),
            ..Default::default()
        };

        assert_eq!(calculate_memory_percentage(&stats), 50.0);
    }

    #[test]
    fn test_calculate_memory_percentage_full_usage() {
        let stats = ContainerStatsResponse {
            memory_stats: Some(ContainerMemoryStats {
                usage: Some(1_000_000_000),
                limit: Some(1_000_000_000),
                max_usage: None,
                stats: None,
                failcnt: None,
                commitbytes: None,
                commitpeakbytes: None,
                privateworkingset: None,
            }),
            ..Default::default()
        };

        assert_eq!(calculate_memory_percentage(&stats), 100.0);
    }

    #[test]
    fn test_calculate_memory_percentage_low_usage() {
        let stats = ContainerStatsResponse {
            memory_stats: Some(ContainerMemoryStats {
                usage: Some(100_000_000),   // 100 MB
                limit: Some(2_000_000_000), // 2 GB
                max_usage: None,
                stats: None,
                failcnt: None,
                commitbytes: None,
                commitpeakbytes: None,
                privateworkingset: None,
            }),
            ..Default::default()
        };

        assert_eq!(calculate_memory_percentage(&stats), 5.0);
    }

    #[test]
    fn test_calculate_memory_percentage_missing_memory_stats() {
        let stats = ContainerStatsResponse {
            memory_stats: None,
            ..Default::default()
        };

        assert_eq!(calculate_memory_percentage(&stats), 0.0);
    }

    #[test]
    fn test_calculate_memory_percentage_missing_usage() {
        let stats = ContainerStatsResponse {
            memory_stats: Some(ContainerMemoryStats {
                usage: None,
                limit: Some(1_000_000_000),
                max_usage: None,
                stats: None,
                failcnt: None,
                commitbytes: None,
                commitpeakbytes: None,
                privateworkingset: None,
            }),
            ..Default::default()
        };

        assert_eq!(calculate_memory_percentage(&stats), 0.0);
    }

    #[test]
    fn test_calculate_memory_percentage_zero_limit() {
        let stats = ContainerStatsResponse {
            memory_stats: Some(ContainerMemoryStats {
                usage: Some(500_000_000),
                limit: Some(0),
                max_usage: None,
                stats: None,
                failcnt: None,
                commitbytes: None,
                commitpeakbytes: None,
                privateworkingset: None,
            }),
            ..Default::default()
        };

        // Should handle division by zero gracefully
        assert_eq!(calculate_memory_percentage(&stats), 0.0);
    }

    #[test]
    fn test_calculate_memory_percentage_missing_limit() {
        let stats = ContainerStatsResponse {
            memory_stats: Some(ContainerMemoryStats {
                usage: Some(500_000_000),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(calculate_memory_percentage(&stats), 0.0);
    }

    #[test]
    fn test_extract_pids_with_limit() {
        let stats = ContainerStatsResponse {
            pids_stats: Some(bollard::models::ContainerPidsStats {
                current: Some(42),
                limit: Some(100),
            }),
            ..Default::default()
        };

        assert_eq!(extract_pids(&stats), (42, 100));
    }

    #[test]
    fn test_extract_pids_no_limit() {
        // A limit of 0 means "no limit"
        let stats = ContainerStatsResponse {
            pids_stats: Some(bollard::models::ContainerPidsStats {
                current: Some(7),
                limit: Some(0),
            }),
            ..Default::default()
        };

        assert_eq!(extract_pids(&stats), (7, 0));
    }

    #[test]
    fn test_extract_pids_missing() {
        let stats = ContainerStatsResponse {
            pids_stats: None,
            ..Default::default()
        };

        assert_eq!(extract_pids(&stats), (0, 0));
    }

    fn create_blkio_entry(op: &str, value: u64) -> ContainerBlkioStatEntry {
        ContainerBlkioStatEntry {
            major: Some(8),
            minor: Some(0),
            op: Some(op.to_string()),
            value: Some(value),
        }
    }

    #[test]
    fn test_extract_disk_bytes_normal() {
        let stats = ContainerStatsResponse {
            blkio_stats: Some(ContainerBlkioStats {
                io_service_bytes_recursive: Some(vec![
                    create_blkio_entry("Read", 1_000_000),
                    create_blkio_entry("Write", 500_000),
                    create_blkio_entry("Sync", 200_000),
                    create_blkio_entry("Async", 100_000),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let (read, write) = extract_disk_bytes(&stats);
        assert_eq!(read, Some(1_000_000));
        assert_eq!(write, Some(500_000));
    }

    #[test]
    fn test_extract_disk_bytes_multiple_devices() {
        let stats = ContainerStatsResponse {
            blkio_stats: Some(ContainerBlkioStats {
                io_service_bytes_recursive: Some(vec![
                    // Device 1
                    ContainerBlkioStatEntry {
                        major: Some(8),
                        minor: Some(0),
                        op: Some("Read".to_string()),
                        value: Some(1_000_000),
                    },
                    ContainerBlkioStatEntry {
                        major: Some(8),
                        minor: Some(0),
                        op: Some("Write".to_string()),
                        value: Some(500_000),
                    },
                    // Device 2
                    ContainerBlkioStatEntry {
                        major: Some(8),
                        minor: Some(16),
                        op: Some("Read".to_string()),
                        value: Some(2_000_000),
                    },
                    ContainerBlkioStatEntry {
                        major: Some(8),
                        minor: Some(16),
                        op: Some("Write".to_string()),
                        value: Some(1_000_000),
                    },
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let (read, write) = extract_disk_bytes(&stats);
        assert_eq!(read, Some(3_000_000)); // 1M + 2M
        assert_eq!(write, Some(1_500_000)); // 500K + 1M
    }

    #[test]
    fn test_extract_disk_bytes_missing_blkio_stats() {
        let stats = ContainerStatsResponse {
            blkio_stats: None,
            ..Default::default()
        };

        let (read, write) = extract_disk_bytes(&stats);
        assert_eq!(read, None);
        assert_eq!(write, None);
    }

    #[test]
    fn test_extract_disk_bytes_empty_entries() {
        let stats = ContainerStatsResponse {
            blkio_stats: Some(ContainerBlkioStats {
                io_service_bytes_recursive: Some(vec![]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let (read, write) = extract_disk_bytes(&stats);
        assert_eq!(read, Some(0));
        assert_eq!(write, Some(0));
    }

    #[test]
    fn test_extract_disk_bytes_missing_recursive() {
        let stats = ContainerStatsResponse {
            blkio_stats: Some(ContainerBlkioStats {
                io_service_bytes_recursive: None,
                ..Default::default()
            }),
            ..Default::default()
        };

        let (read, write) = extract_disk_bytes(&stats);
        assert_eq!(read, None);
        assert_eq!(write, None);
    }

    #[test]
    fn test_calculate_disk_rates_normal() {
        let stats = ContainerStatsResponse {
            blkio_stats: Some(ContainerBlkioStats {
                io_service_bytes_recursive: Some(vec![
                    create_blkio_entry("Read", 2_000_000),
                    create_blkio_entry("Write", 1_000_000),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };

        // Simulate 1 second elapsed with previous values
        let prev_read = Some(1_000_000u64);
        let prev_write = Some(500_000u64);
        let prev_time = Some(Instant::now() - std::time::Duration::from_secs(1));

        let (current_read, current_write) = extract_disk_bytes(&stats);
        let (read_rate, write_rate) = calculate_disk_rates(
            current_read,
            current_write,
            prev_read,
            prev_write,
            prev_time,
        );

        // Read: 2M - 1M = 1M bytes in ~1 second = ~1MB/s
        // Write: 1M - 500K = 500K bytes in ~1 second = ~500KB/s
        // Allow some tolerance for timing
        assert!(read_rate > 900_000.0 && read_rate < 1_100_000.0);
        assert!(write_rate > 450_000.0 && write_rate < 550_000.0);
    }

    #[test]
    fn test_calculate_disk_rates_no_previous() {
        let stats = ContainerStatsResponse {
            blkio_stats: Some(ContainerBlkioStats {
                io_service_bytes_recursive: Some(vec![
                    create_blkio_entry("Read", 1_000_000),
                    create_blkio_entry("Write", 500_000),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let (current_read, current_write) = extract_disk_bytes(&stats);
        let (read_rate, write_rate) =
            calculate_disk_rates(current_read, current_write, None, None, None);
        assert_eq!(read_rate, 0.0);
        assert_eq!(write_rate, 0.0);
    }

    #[test]
    fn test_calculate_disk_rates_missing_current() {
        let stats = ContainerStatsResponse {
            blkio_stats: None,
            ..Default::default()
        };

        let prev_time = Some(Instant::now() - std::time::Duration::from_secs(1));
        let (current_read, current_write) = extract_disk_bytes(&stats);
        let (read_rate, write_rate) = calculate_disk_rates(
            current_read,
            current_write,
            Some(1_000_000),
            Some(500_000),
            prev_time,
        );

        assert_eq!(read_rate, 0.0);
        assert_eq!(write_rate, 0.0);
    }

    #[test]
    fn test_parse_io_stat_content() {
        let content = "8:0 rbytes=20480 wbytes=891289600 rios=2 wios=1700\n253:0 rbytes=10240 wbytes=10000 rios=1 wios=1";
        let (read, write) = parse_io_stat_content(content);
        assert_eq!(read, Some(30720));
        assert_eq!(write, Some(891299600));
    }

    /// cgroupfs dirs carry the full ID, and a miss isn't re-scanned every frame
    #[test]
    fn test_cgroup_v2_lookup_matches_full_id_and_caches_a_miss() {
        let root = std::env::temp_dir().join(format!("dtop-cgroup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let full_id = format!("abcdef123456{}", "0".repeat(52));
        let dir = root.join("docker").join(&full_id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("io.stat"), "8:0 rbytes=10 wbytes=20 rios=1 wios=1").unwrap();

        let mut cache = None;
        assert_eq!(
            extract_cgroup_v2_disk_bytes(&root, "abcdef123456", &mut cache),
            (Some(10), Some(20))
        );

        let mut miss = None;
        assert_eq!(
            extract_cgroup_v2_disk_bytes(&root, "fedcba654321", &mut miss),
            (None, None)
        );
        assert_eq!(miss, Some(None));
        // Created after the miss: not looked for again
        let late = root
            .join("docker")
            .join(format!("fedcba654321{}", "0".repeat(52)));
        std::fs::create_dir_all(&late).unwrap();
        std::fs::write(late.join("io.stat"), "8:0 rbytes=1 wbytes=1").unwrap();
        assert_eq!(
            extract_cgroup_v2_disk_bytes(&root, "fedcba654321", &mut miss),
            (None, None)
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
