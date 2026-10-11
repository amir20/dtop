//! Samples the metrics of the machine dtop runs on (CPU, memory, swap, load,
//! uptime) for the system stats header.
//!
//! This reads the local OS directly rather than going through Docker, which is
//! why it only describes the machine behind the `local` host. Remote hosts get
//! their numbers from `docker info` plus their containers' stats instead.

use std::time::{Duration, Instant};

use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, Networks, System};

use crate::core::types::{AppEvent, EventSender, SystemStats};

/// How often a new sample is taken. CPU usage is averaged over this window.
pub(crate) const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);

/// Sends a [`AppEvent::SystemStats`] every [`SAMPLE_INTERVAL`] until the UI goes away.
///
/// Blocking: run it on its own thread, like the keyboard worker. `sysinfo`
/// refreshes are synchronous syscalls (a few ms for networks and memory on
/// macOS) that shouldn't occupy a tokio worker.
pub fn system_stats_worker(tx: EventSender) {
    let hostname = System::host_name().unwrap_or_else(|| "localhost".to_string());
    let mut sys = System::new();
    let mut networks = Networks::new_with_refreshed_list();

    // CPU usage and network traffic are deltas between two refreshes, so the
    // first refresh only primes the counters and the first real sample has to
    // wait a little.
    sys.refresh_cpu_usage();
    let mut last_refresh = Instant::now();
    std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);

    loop {
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        // `true` drops interfaces that went away (VPNs, container veths)
        networks.refresh(true);
        let elapsed = last_refresh.elapsed().as_secs_f64();
        last_refresh = Instant::now();

        let stats = sample(&sys, &networks, elapsed, &hostname);
        if tx.blocking_send(AppEvent::SystemStats(stats)).is_err() {
            return;
        }

        std::thread::sleep(SAMPLE_INTERVAL);
    }
}

/// Builds a [`SystemStats`] from an already-refreshed [`System`] and [`Networks`],
/// `elapsed` seconds after the previous refresh.
fn sample(sys: &System, networks: &Networks, elapsed: f64, hostname: &str) -> SystemStats {
    let load = System::load_average();

    let (mut rx, mut tx) = (0u64, 0u64);
    for (name, data) in networks.iter() {
        if !is_virtual_interface(name) {
            rx += data.received();
            tx += data.transmitted();
        }
    }
    let rate = |bytes: u64| {
        if elapsed > 0.0 {
            bytes as f64 / elapsed
        } else {
            0.0
        }
    };

    SystemStats {
        hostname: hostname.to_string(),
        cpu: f64::from(sys.global_cpu_usage()),
        cpus: sys.cpus().len(),
        memory_used_bytes: sys.used_memory(),
        memory_total_bytes: sys.total_memory(),
        swap_used_bytes: sys.used_swap(),
        swap_total_bytes: sys.total_swap(),
        net_rx_bytes_per_sec: rate(rx),
        net_tx_bytes_per_sec: rate(tx),
        load_average: [load.one, load.five, load.fifteen],
        uptime_secs: System::uptime(),
    }
}

/// Interfaces whose traffic doesn't leave the machine, or that carry traffic
/// already counted on a physical interface.
///
/// Loopback never leaves the machine (and on macOS carries the Docker VM's
/// chatter with the host). Container `veth`s and the Docker bridges see every
/// container packet once more on its way to the real NIC, so counting them
/// would roughly double container traffic.
pub(crate) fn is_virtual_interface(name: &str) -> bool {
    // `lo` (Linux) and `lo0` (macOS), but not e.g. `lowpan0`
    let loopback = name
        .strip_prefix("lo")
        .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()));
    loopback
        || name.starts_with("veth")
        || name == "docker0"
        || name == "docker_gwbridge"
        || name.starts_with("br-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_reads_this_machine() {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        let networks = Networks::new_with_refreshed_list();

        let stats = sample(&sys, &networks, 0.0, "box");

        assert_eq!(stats.hostname, "box");
        assert!(stats.cpus > 0);
        assert!(stats.memory_total_bytes > 0);
        assert!(stats.memory_used_bytes <= stats.memory_total_bytes);
        assert!((0.0..=100.0).contains(&stats.cpu));
        // No time has passed since the previous refresh, so there is no rate yet
        assert_eq!(stats.net_rx_bytes_per_sec, 0.0);
    }

    #[test]
    fn virtual_interfaces_are_skipped() {
        for name in [
            "lo",
            "lo0",
            "veth1a2b3c",
            "docker0",
            "docker_gwbridge",
            "br-0123abcd",
        ] {
            assert!(is_virtual_interface(name), "{name}");
        }
        for name in [
            "eth0",
            "en0",
            "wlan0",
            "ens5",
            "utun3",
            "lowpan0",
            "dockerhost0",
        ] {
            assert!(!is_virtual_interface(name), "{name}");
        }
    }
}
