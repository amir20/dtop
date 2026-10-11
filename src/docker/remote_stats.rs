//! Samples a remote SSH host's own CPU, memory, swap, load, uptime and network
//! by reading `/proc` over the host's existing SSH connection.
//!
//! The Docker API only describes containers and the daemon's capacity, so this
//! is what gives `ssh://` hosts a `System` line like the local machine's. It only
//! works on Linux hosts; anywhere else the first read fails and sampling stops.

use std::time::{Duration, Instant};

use crate::core::system_stats::{SAMPLE_INTERVAL, is_virtual_interface};
use crate::core::types::{AppEvent, EventSender, HostId, SystemStats};
use crate::docker::ssh::SshRunner;

/// Sections are separated by a marker line so each can be parsed on its own.
const SEPARATOR: &str = "@@dtop@@";

/// One round trip for everything. `exec` keeps the remote shell from lingering.
const SCRIPT: &str = "cat /proc/stat && echo @@dtop@@ && cat /proc/meminfo && echo @@dtop@@ \
                      && cat /proc/loadavg && echo @@dtop@@ && cat /proc/uptime && echo @@dtop@@ \
                      && exec cat /proc/net/dev";

/// Wait after a failed connection before trying again, so a host that is down
/// isn't hammered with SSH handshakes.
const RETRY_DELAY: Duration = Duration::from_secs(10);

/// Sends [`AppEvent::RemoteSystemStats`] for `host_id` until the UI goes away or
/// the host turns out not to have a Linux `/proc`.
pub async fn remote_system_stats_worker(host_id: HostId, ssh: SshRunner, tx: EventSender) {
    let mut previous: Option<Snapshot> = None;

    loop {
        let delay = match ssh.run_sh(SCRIPT).await {
            Ok(output) if output.status.success() => {
                let Some(snapshot) = Snapshot::parse(&String::from_utf8_lossy(&output.stdout))
                else {
                    tracing::warn!(
                        "Couldn't parse /proc on '{host_id}'; not showing its system stats"
                    );
                    return;
                };
                // Rates need two samples, so the first one only primes them
                if let Some(previous) = &previous {
                    let stats = snapshot.stats_since(previous, &host_id);
                    if tx
                        .send(AppEvent::RemoteSystemStats(host_id.clone(), stats))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                previous = Some(snapshot);
                SAMPLE_INTERVAL
            }
            Ok(output) => {
                // The command ran but `/proc` isn't there: not Linux. Won't change.
                tracing::info!(
                    "'{host_id}' has no readable /proc ({}); not showing its system stats",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
                return;
            }
            Err(e) => {
                tracing::debug!("Failed to sample system stats on '{host_id}': {e}");
                // Counters from before an outage would produce a bogus rate
                previous = None;
                RETRY_DELAY
            }
        };

        if tx.is_closed() {
            return;
        }
        tokio::time::sleep(delay).await;
    }
}

/// Raw counters from one read of `/proc`.
#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    at: Instant,
    /// All CPU time (jiffies) from the aggregate `cpu` line, and the idle part
    cpu_total: u64,
    cpu_idle: u64,
    cpus: usize,
    memory_total_bytes: u64,
    memory_available_bytes: u64,
    swap_total_bytes: u64,
    swap_free_bytes: u64,
    load_average: [f64; 3],
    uptime_secs: u64,
    net_rx_bytes: u64,
    net_tx_bytes: u64,
}

impl Snapshot {
    fn parse(output: &str) -> Option<Self> {
        let mut sections = output.split(SEPARATOR);
        let (stat, meminfo, loadavg, uptime, net_dev) = (
            sections.next()?,
            sections.next()?,
            sections.next()?,
            sections.next()?,
            sections.next()?,
        );

        // cpu  user nice system idle iowait irq softirq steal guest guest_nice
        // guest time is already included in user, so only the first 8 count.
        let cpu: Vec<u64> = stat
            .lines()
            .find(|l| l.starts_with("cpu "))?
            .split_whitespace()
            .skip(1)
            .take(8)
            .map(|v| v.parse().ok())
            .collect::<Option<_>>()?;
        if cpu.len() < 5 {
            return None;
        }
        let cpus = stat
            .lines()
            .filter(|l| l.starts_with("cpu") && l.as_bytes().get(3).is_some_and(u8::is_ascii_digit))
            .count();

        let meminfo_kb = |key: &str| -> Option<u64> {
            meminfo
                .lines()
                .find_map(|l| l.strip_prefix(key)?.strip_prefix(':'))?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
                .map(|kb| kb * 1024)
        };
        let memory_total_bytes = meminfo_kb("MemTotal")?;
        // MemAvailable is missing on kernels older than 3.14
        let memory_available_bytes =
            meminfo_kb("MemAvailable").or_else(|| meminfo_kb("MemFree"))?;

        let mut load = loadavg.split_whitespace().map(|v| v.parse::<f64>().ok());
        let load_average = [load.next()??, load.next()??, load.next()??];

        let uptime_secs = uptime.split_whitespace().next()?.parse::<f64>().ok()? as u64;

        // Inter-|   Receive ...                    |  Transmit
        //  face |bytes packets errs drop fifo frame compressed multicast|bytes ...
        //   eth0: 1234 ...
        let (mut net_rx_bytes, mut net_tx_bytes) = (0, 0);
        for line in net_dev.lines() {
            let Some((name, counters)) = line.split_once(':') else {
                continue;
            };
            if is_virtual_interface(name.trim()) {
                continue;
            }
            let fields: Vec<&str> = counters.split_whitespace().collect();
            if let (Some(rx), Some(tx)) = (fields.first(), fields.get(8)) {
                net_rx_bytes += rx.parse::<u64>().unwrap_or(0);
                net_tx_bytes += tx.parse::<u64>().unwrap_or(0);
            }
        }

        Some(Self {
            at: Instant::now(),
            cpu_total: cpu.iter().sum(),
            cpu_idle: cpu[3] + cpu[4],
            cpus,
            memory_total_bytes,
            memory_available_bytes,
            swap_total_bytes: meminfo_kb("SwapTotal").unwrap_or(0),
            swap_free_bytes: meminfo_kb("SwapFree").unwrap_or(0),
            load_average,
            uptime_secs,
            net_rx_bytes,
            net_tx_bytes,
        })
    }

    /// Turns two snapshots into usage figures, labelled `host_id`.
    fn stats_since(&self, previous: &Self, host_id: &str) -> SystemStats {
        let total = self.cpu_total.saturating_sub(previous.cpu_total);
        let idle = self.cpu_idle.saturating_sub(previous.cpu_idle);
        let cpu = if total > 0 {
            (total.saturating_sub(idle)) as f64 / total as f64 * 100.0
        } else {
            0.0
        };

        let elapsed = self.at.duration_since(previous.at).as_secs_f64();
        // Counters reset when an interface goes away; saturate rather than wrap
        let rate = |now: u64, before: u64| {
            if elapsed > 0.0 {
                now.saturating_sub(before) as f64 / elapsed
            } else {
                0.0
            }
        };

        SystemStats {
            hostname: host_id.to_string(),
            cpu,
            cpus: self.cpus,
            memory_used_bytes: self
                .memory_total_bytes
                .saturating_sub(self.memory_available_bytes),
            memory_total_bytes: self.memory_total_bytes,
            swap_used_bytes: self.swap_total_bytes.saturating_sub(self.swap_free_bytes),
            swap_total_bytes: self.swap_total_bytes,
            net_rx_bytes_per_sec: rate(self.net_rx_bytes, previous.net_rx_bytes),
            net_tx_bytes_per_sec: rate(self.net_tx_bytes, previous.net_tx_bytes),
            load_average: self.load_average,
            uptime_secs: self.uptime_secs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(cpu: &str, rx: u64, tx: u64) -> String {
        format!(
            "cpu  {cpu}\n\
             cpu0 1 2 3 4 5 6 7 8 0 0\n\
             cpu1 1 2 3 4 5 6 7 8 0 0\n\
             intr 12345\n\
             {SEPARATOR}\n\
             MemTotal:        8000000 kB\n\
             MemFree:          500000 kB\n\
             MemAvailable:    6000000 kB\n\
             SwapTotal:       2000000 kB\n\
             SwapFree:        1500000 kB\n\
             {SEPARATOR}\n\
             0.52 0.40 0.31 2/345 6789\n\
             {SEPARATOR}\n\
             273600.25 1000000.00\n\
             {SEPARATOR}\n\
             Inter-|   Receive                                                |  Transmit\n \
             face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n    \
             lo: 999999 10 0 0 0 0 0 0 999999 10 0 0 0 0 0 0\n  \
             eth0: {rx} 10 0 0 0 0 0 0 {tx} 10 0 0 0 0 0 0\n\
             docker0: 555 10 0 0 0 0 0 0 555 10 0 0 0 0 0 0\n\
             veth12ab: 777 10 0 0 0 0 0 0 777 10 0 0 0 0 0 0\n"
        )
    }

    #[test]
    fn parses_proc() {
        let snapshot = Snapshot::parse(&output("100 0 50 800 50 0 0 0 0 0", 1000, 2000)).unwrap();

        assert_eq!(snapshot.cpu_total, 1000);
        assert_eq!(snapshot.cpu_idle, 850);
        assert_eq!(snapshot.cpus, 2);
        assert_eq!(snapshot.memory_total_bytes, 8_000_000 * 1024);
        assert_eq!(snapshot.memory_available_bytes, 6_000_000 * 1024);
        assert_eq!(snapshot.swap_free_bytes, 1_500_000 * 1024);
        assert_eq!(snapshot.load_average, [0.52, 0.40, 0.31]);
        assert_eq!(snapshot.uptime_secs, 273_600);
        // Only eth0: loopback, the bridge and the veth are skipped
        assert_eq!((snapshot.net_rx_bytes, snapshot.net_tx_bytes), (1000, 2000));
    }

    #[test]
    fn rates_come_from_the_difference_between_snapshots() {
        let mut before = Snapshot::parse(&output("100 0 50 800 50 0 0 0 0 0", 1000, 2000)).unwrap();
        let after = Snapshot::parse(&output("400 0 150 1100 50 0 0 0 0 0", 5000, 3000)).unwrap();
        before.at = after.at - Duration::from_secs(2);

        let stats = after.stats_since(&before, "user@server1");

        // 700 jiffies elapsed, 300 of them idle
        assert!((stats.cpu - 400.0 / 700.0 * 100.0).abs() < 1e-9);
        assert_eq!(stats.hostname, "user@server1");
        assert_eq!(stats.memory_used_bytes, 2_000_000 * 1024);
        assert_eq!(stats.swap_used_bytes, 500_000 * 1024);
        assert_eq!(stats.net_rx_bytes_per_sec, 2000.0);
        assert_eq!(stats.net_tx_bytes_per_sec, 500.0);
    }

    #[test]
    fn rejects_output_without_proc() {
        assert_eq!(Snapshot::parse(""), None);
        assert_eq!(
            Snapshot::parse("cat: /proc/stat: No such file or directory"),
            None
        );
    }
}
