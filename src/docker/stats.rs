use bollard::models::ContainerStatsResponse;
use bollard::query_parameters::StatsOptions;
use futures_util::stream::StreamExt;
use std::time::Instant;

use crate::core::types::{AppEvent, ContainerKey, ContainerStats, EventSender};
use crate::docker::connection::DockerHost;

/// Streams stats for a single container and sends updates via the event channel
///
/// Uses exponential decay smoothing to reduce noise in stats:
/// smoothed = alpha * new_value + (1 - alpha) * previous_smoothed
///
/// # Arguments
/// * `host` - Docker host instance with identifier
/// * `truncated_id` - Truncated container ID (12 chars) - Docker API accepts partial IDs
/// * `tx` - Event sender channel
pub async fn stream_container_stats(host: DockerHost, truncated_id: String, tx: EventSender) {
    let stats_options = StatsOptions {
        stream: true,
        one_shot: false,
    };

    let mut stats_stream = host.docker.stats(&truncated_id, Some(stats_options));

    // Smoothing factor: higher alpha = more responsive, lower alpha = smoother
    // 0.3 provides good balance between responsiveness and smoothness
    const ALPHA: f64 = 0.3;

    let mut smoothed_cpu: Option<f64> = None;
    let mut smoothed_memory: Option<f64> = None;
    let mut smoothed_net_tx: Option<f64> = None;
    let mut smoothed_net_rx: Option<f64> = None;
    let mut smoothed_disk_read: Option<f64> = None;
    let mut smoothed_disk_write: Option<f64> = None;

    // Track previous network stats for rate calculation
    let mut prev_net_tx: Option<u64> = None;
    let mut prev_net_rx: Option<u64> = None;
    let mut prev_timestamp: Option<Instant> = None;

    // Track previous disk I/O stats for rate calculation
    let mut prev_disk_read: Option<u64> = None;
    let mut prev_disk_write: Option<u64> = None;

    // Check if host is local before permitting local cgroups v2 filesystem fallbacks
    let is_local_host = host.host_id == "local" || host.host_id.starts_with("unix://");
    let mut cached_cgroup_path: Option<std::path::PathBuf> = None;

    while let Some(result) = stats_stream.next().await {
        match result {
            Ok(stats) => {
                let cpu_percent = calculate_cpu_percentage(&stats);
                let memory_percent = calculate_memory_percentage(&stats);
                let (net_tx_rate, net_rx_rate) =
                    calculate_network_rates(&stats, prev_net_tx, prev_net_rx, prev_timestamp);
                let (disk_read_rate, disk_write_rate) = calculate_disk_rates(
                    &stats,
                    &truncated_id,
                    is_local_host,
                    &mut cached_cgroup_path,
                    prev_disk_read,
                    prev_disk_write,
                    prev_timestamp,
                );

                // Update previous network values for next iteration
                let (tx_bytes, rx_bytes) = extract_network_bytes(&stats);
                prev_net_tx = tx_bytes;
                prev_net_rx = rx_bytes;

                // Update previous disk I/O values for next iteration
                let (read_bytes, write_bytes) = extract_disk_bytes_with_cgroup_fallback(
                    &stats,
                    &truncated_id,
                    is_local_host,
                    &mut cached_cgroup_path,
                );
                prev_disk_read = read_bytes;
                prev_disk_write = write_bytes;

                prev_timestamp = Some(Instant::now());

                // Apply exponential moving average (first value passes through unsmoothed)
                let cpu = ema(smoothed_cpu, cpu_percent, ALPHA);
                let memory = ema(smoothed_memory, memory_percent, ALPHA);
                let network_tx_bytes_per_sec = ema(smoothed_net_tx, net_tx_rate, ALPHA);
                let network_rx_bytes_per_sec = ema(smoothed_net_rx, net_rx_rate, ALPHA);
                let disk_read_bytes_per_sec = ema(smoothed_disk_read, disk_read_rate, ALPHA);
                let disk_write_bytes_per_sec = ema(smoothed_disk_write, disk_write_rate, ALPHA);

                // Update smoothed values for next iteration
                smoothed_cpu = Some(cpu);
                smoothed_memory = Some(memory);
                smoothed_net_tx = Some(network_tx_bytes_per_sec);
                smoothed_net_rx = Some(network_rx_bytes_per_sec);
                smoothed_disk_read = Some(disk_read_bytes_per_sec);
                smoothed_disk_write = Some(disk_write_bytes_per_sec);

                // Extract raw memory bytes for display
                let (memory_used_bytes, memory_limit_bytes) = extract_memory_bytes(&stats);

                let stats = ContainerStats {
                    cpu,
                    memory,
                    memory_used_bytes,
                    memory_limit_bytes,
                    network_tx_bytes_per_sec,
                    network_rx_bytes_per_sec,
                    disk_read_bytes_per_sec,
                    disk_write_bytes_per_sec,
                };

                let key = ContainerKey::new(host.host_id.clone(), truncated_id.clone());
                if tx.send(AppEvent::ContainerStat(key, stats)).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    // Stats stream ended (container stopped or network hiccup).
    // Don't send ContainerDestroyed — the container may still be running.
    // Docker events (die/stop/destroy) handle container lifecycle correctly.
    tracing::debug!(
        "Stats stream ended for container {} on host {}",
        truncated_id,
        host.host_id
    );
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
    let cpu_stats = match &stats.cpu_stats {
        Some(cs) => cs,
        None => return 0.0,
    };
    let precpu_stats = match &stats.precpu_stats {
        Some(pcs) => pcs,
        None => return 0.0,
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
    let memory_stats = match &stats.memory_stats {
        Some(ms) => ms,
        None => return 0.0,
    };

    let memory_usage = calculate_used_memory(memory_stats);
    let memory_limit = memory_stats.limit.unwrap_or(1) as f64;

    if memory_limit > 0.0 {
        (memory_usage / memory_limit) * 100.0
    } else {
        0.0
    }
}

/// Extracts memory bytes (used, limit) from container stats
/// Subtracts cache to match `docker stats` behavior
fn extract_memory_bytes(stats: &ContainerStatsResponse) -> (u64, u64) {
    let memory_stats = match &stats.memory_stats {
        Some(ms) => ms,
        None => return (0, 0),
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

/// Extracts total network bytes (tx, rx) from container stats
fn extract_network_bytes(stats: &ContainerStatsResponse) -> (Option<u64>, Option<u64>) {
    let networks = match &stats.networks {
        Some(nets) => nets,
        None => return (None, None),
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
    let (prev_tx, prev_rx, prev_time) = match (prev_tx, prev_rx, prev_time) {
        (Some(tx), Some(rx), Some(time)) => (tx, rx, time),
        _ => return (0.0, 0.0),
    };

    let (current_tx, current_rx) = match (current_tx, current_rx) {
        (Some(tx), Some(rx)) => (tx, rx),
        _ => return (0.0, 0.0),
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
    let blkio_stats = match &stats.blkio_stats {
        Some(bs) => bs,
        None => return (None, None),
    };

    let entries = match blkio_stats
        .io_service_bytes_recursive
        .as_ref()
        .or_else(|| blkio_stats.io_serviced_recursive.as_ref())
    {
        Some(e) => e,
        None => return (None, None),
    };

    let mut total_read = 0u64;
    let mut total_write = 0u64;

    for entry in entries {
        let value = entry.value.unwrap_or(0);
        if let Some(op) = entry.op.as_deref() {
            if op.eq_ignore_ascii_case("read") || op.eq_ignore_ascii_case("r") {
                total_read += value;
            } else if op.eq_ignore_ascii_case("write") || op.eq_ignore_ascii_case("w") {
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
            if let Some(r) = part.strip_prefix("rbytes=") {
                if let Ok(val) = r.parse::<u64>() {
                    total_read = total_read.saturating_add(val);
                    found = true;
                }
            } else if let Some(w) = part.strip_prefix("wbytes=") {
                if let Ok(val) = w.parse::<u64>() {
                    total_write = total_write.saturating_add(val);
                    found = true;
                }
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
/// Uses direct path lookups first before falling back to systemd directory scanning,
/// caches resolved path for stream efficiency, and uses strict container ID prefix matching.
fn extract_cgroup_v2_disk_bytes(
    container_id: &str,
    cached_path: &mut Option<std::path::PathBuf>,
) -> (Option<u64>, Option<u64>) {
    // 1. Try cached path first
    if let Some(path) = cached_path {
        if let Ok(content) = std::fs::read_to_string(path) {
            return parse_io_stat_content(&content);
        }
    }

    // 2. Try direct cgroupfs path
    let cgroupfs_path =
        std::path::PathBuf::from(format!("/sys/fs/cgroup/docker/{}/io.stat", container_id));
    if let Ok(content) = std::fs::read_to_string(&cgroupfs_path) {
        *cached_path = Some(cgroupfs_path);
        return parse_io_stat_content(&content);
    }

    // 3. Scan system.slice for systemd cgroup scope matching docker-<container_id>*.scope
    if let Ok(entries) = std::fs::read_dir("/sys/fs/cgroup/system.slice") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(scope_id) = name.strip_prefix("docker-") {
                if scope_id.starts_with(container_id) && name.ends_with(".scope") {
                    let io_stat_path = entry.path().join("io.stat");
                    if let Ok(content) = std::fs::read_to_string(&io_stat_path) {
                        let res = parse_io_stat_content(&content);
                        if res.0.is_some() || res.1.is_some() {
                            *cached_path = Some(io_stat_path);
                            return res;
                        }
                    }
                }
            }
        }
    }

    (None, None)
}

fn extract_disk_bytes_with_cgroup_fallback(
    stats: &ContainerStatsResponse,
    container_id: &str,
    is_local_host: bool,
    cached_cgroup_path: &mut Option<std::path::PathBuf>,
) -> (Option<u64>, Option<u64>) {
    let (read, write) = extract_disk_bytes(stats);
    if read.is_some() || write.is_some() {
        (read, write)
    } else if is_local_host {
        extract_cgroup_v2_disk_bytes(container_id, cached_cgroup_path)
    } else {
        (None, None)
    }
}

/// Calculates disk I/O rates in bytes per second
fn calculate_disk_rates(
    stats: &ContainerStatsResponse,
    container_id: &str,
    is_local_host: bool,
    cached_cgroup_path: &mut Option<std::path::PathBuf>,
    prev_read: Option<u64>,
    prev_write: Option<u64>,
    prev_time: Option<Instant>,
) -> (f64, f64) {
    let (current_read, current_write) = extract_disk_bytes_with_cgroup_fallback(
        stats,
        container_id,
        is_local_host,
        cached_cgroup_path,
    );

    // If we don't have previous values, return 0
    let (prev_read, prev_write, prev_time) = match (prev_read, prev_write, prev_time) {
        (Some(r), Some(w), Some(t)) => (r, w, t),
        _ => return (0.0, 0.0),
    };

    let (current_read, current_write) = match (current_read, current_write) {
        (Some(r), Some(w)) => (r, w),
        _ => return (0.0, 0.0),
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
        let mut cached_path = None;

        let (read_rate, write_rate) =
            calculate_disk_rates(&stats, "test-id", false, &mut cached_path, prev_read, prev_write, prev_time);

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

        let mut cached_path = None;
        let (read_rate, write_rate) = calculate_disk_rates(&stats, "test-id", false, &mut cached_path, None, None, None);
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
        let mut cached_path = None;
        let (read_rate, write_rate) =
            calculate_disk_rates(&stats, "test-id", false, &mut cached_path, Some(1_000_000), Some(500_000), prev_time);

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
}
