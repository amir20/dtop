//! Samples a remote SSH host's own CPU, memory, swap, load, uptime and network
//! by reading `/proc` over the host's existing SSH connection.
//!
//! The Docker API only describes containers and the daemon's capacity, so this
//! is what gives `ssh://` hosts a `System` line like the local machine's. It only
//! works on Linux hosts; anywhere else the reads keep failing and sampling stops.

use std::future::Future;
use std::io;
use std::process::Output;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::core::system_stats::{SAMPLE_INTERVAL, is_virtual_interface};
use crate::core::types::{AppEvent, EventSender, HostId, SystemStats};
use crate::docker::connection::DockerHost;
use crate::docker::ssh::SshRunner;

/// Sections are separated by a marker line so each can be parsed on its own.
const SEPARATOR: &str = "@@dtop@@";

/// One round trip for everything. `exec` keeps the remote shell from lingering.
const SCRIPT: &str = "cat /proc/stat && echo @@dtop@@ && cat /proc/meminfo && echo @@dtop@@ \
                      && cat /proc/loadavg && echo @@dtop@@ && cat /proc/uptime && echo @@dtop@@ \
                      && exec cat /proc/net/dev";

/// First wait after a failed read; doubles per failure in a row up to
/// [`MAX_RETRY_DELAY`], so a host that is down isn't hammered with SSH
/// handshakes.
const RETRY_DELAY: Duration = Duration::from_secs(10);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

/// Reads in a row that find no `/proc` before giving up on the host (a remote
/// that isn't Linux). Every other failure — connection trouble, a `cat` killed
/// under load, truncated output — is retried for as long as dtop runs, so a
/// bad moment never costs a Linux host its System line for the session.
const MAX_MISSING_PROC_READS: u32 = 3;

/// Runs the sampling script on a host. [`SshRunner`] in production; a trait so
/// the worker's retry and give-up behavior can be tested without SSH.
pub trait RemoteShell: Send + Sync + 'static {
    /// See [`SshRunner::run_sh`]: `Err` for connection trouble, `Ok` with the
    /// exit status for a script that ran.
    fn run_sh(&self, script: &str) -> impl Future<Output = io::Result<Output>> + Send;
}

impl RemoteShell for SshRunner {
    fn run_sh(&self, script: &str) -> impl Future<Output = io::Result<Output>> + Send {
        SshRunner::run_sh(self, script)
    }
}

/// Starts sampling `host`'s machine if it is reached over SSH. Runs for the
/// rest of the session and rides out outages on its own.
pub fn spawn_for_host(host: &DockerHost, tx: &EventSender, header_visible: &Arc<AtomicBool>) {
    if let Some(ssh) = host.ssh.clone() {
        tokio::spawn(remote_system_stats_worker(
            host.host_id.clone(),
            ssh,
            tx.clone(),
            Arc::clone(header_visible),
        ));
    }
}

/// Sends [`AppEvent::RemoteSystemStats`] for `host_id` until the UI goes away or
/// the host turns out not to have a readable Linux `/proc`.
///
/// Idles while `header_visible` is false: with the header hidden ('t') nothing
/// shows the result, and each read costs an SSH channel and a process launch.
pub async fn remote_system_stats_worker(
    host_id: HostId,
    shell: impl RemoteShell,
    tx: EventSender,
    header_visible: Arc<AtomicBool>,
) {
    let mut previous: Option<Snapshot> = None;
    let mut failures_in_a_row: u32 = 0;
    let mut missing_proc_reads: u32 = 0;

    loop {
        if !header_visible.load(Ordering::Relaxed) {
            // Rates across the pause would be averaged over it; start fresh
            previous = None;
            if !wait(&tx, SAMPLE_INTERVAL).await {
                return;
            }
            continue;
        }

        let snapshot = match shell.run_sh(SCRIPT).await {
            Ok(output) if output.status.success() => {
                Snapshot::parse(&String::from_utf8_lossy(&output.stdout))
                    .ok_or_else(|| format!("couldn't parse /proc on '{host_id}'"))
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // `cat: /proc/stat: No such file or directory`: not Linux
                if stderr.contains("No such file") {
                    missing_proc_reads += 1;
                    if missing_proc_reads >= MAX_MISSING_PROC_READS {
                        tracing::info!(
                            "'{host_id}' has no /proc ({}); not showing its system stats",
                            stderr.trim()
                        );
                        return;
                    }
                } else {
                    missing_proc_reads = 0;
                }
                Err(format!(
                    "reading /proc on '{host_id}' failed ({}): {}",
                    output.status,
                    stderr.trim()
                ))
            }
            Err(e) => Err(format!("failed to sample system stats on '{host_id}': {e}")),
        };

        let delay = match snapshot {
            Ok(snapshot) => {
                failures_in_a_row = 0;
                missing_proc_reads = 0;
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
            Err(reason) => {
                // Counters from before a failure would produce a bogus rate
                previous = None;
                failures_in_a_row += 1;
                let delay = retry_delay(failures_in_a_row);
                tracing::debug!("{reason}; retrying in {delay:?}");
                delay
            }
        };

        if !wait(&tx, delay).await {
            return;
        }
    }
}

/// [`RETRY_DELAY`] doubled for each failure in a row after the first, capped.
fn retry_delay(failures_in_a_row: u32) -> Duration {
    let doublings = failures_in_a_row.saturating_sub(1).min(8);
    (RETRY_DELAY * 2u32.pow(doublings)).min(MAX_RETRY_DELAY)
}

/// Sleeps for `delay`, returning `false` if the UI went away meanwhile.
async fn wait(tx: &EventSender, delay: Duration) -> bool {
    if tx.is_closed() {
        return false;
    }
    tokio::time::sleep(delay).await;
    !tx.is_closed()
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
        // MemAvailable is missing on kernels older than 3.14. MemFree counts
        // reclaimable cache as used, so usage reads high there; accepted.
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
    use tokio::sync::mpsc;

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

    /// Plays back scripted results, then repeats the last one.
    struct FakeShell(std::sync::Mutex<std::collections::VecDeque<io::Result<Output>>>);

    impl FakeShell {
        fn new(results: Vec<io::Result<Output>>) -> Self {
            Self(std::sync::Mutex::new(results.into()))
        }
    }

    impl RemoteShell for FakeShell {
        async fn run_sh(&self, _script: &str) -> io::Result<Output> {
            let mut results = self.0.lock().unwrap();
            if results.len() > 1 {
                results.pop_front().unwrap()
            } else {
                match results.front().unwrap() {
                    Ok(output) => Ok(output.clone()),
                    Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
                }
            }
        }
    }

    fn exited(code: i32, stdout: &str) -> io::Result<Output> {
        exited_with_stderr(code, stdout, "")
    }

    fn exited_with_stderr(code: i32, stdout: &str, stderr: &str) -> io::Result<Output> {
        use std::os::unix::process::ExitStatusExt;
        Ok(Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        })
    }

    fn proc_read() -> io::Result<Output> {
        exited(0, &output("100 0 50 800 50 0 0 0 0 0", 1000, 2000))
    }

    fn connection_error() -> io::Result<Output> {
        Err(io::Error::new(io::ErrorKind::ConnectionRefused, "refused"))
    }

    /// Runs the worker to completion or until it has sent `wanted` samples.
    async fn run_worker(shell: FakeShell, wanted: usize) -> (usize, bool) {
        let (tx, mut rx) = mpsc::channel(16);
        let worker = tokio::spawn(remote_system_stats_worker(
            "h".to_string(),
            shell,
            tx,
            Arc::new(AtomicBool::new(true)),
        ));
        let mut received = 0;
        while received < wanted {
            match rx.recv().await {
                Some(AppEvent::RemoteSystemStats(..)) => received += 1,
                Some(_) => {}
                None => break, // worker returned
            }
        }
        let finished = worker.is_finished();
        worker.abort();
        (received, finished)
    }

    /// Hidden header: no SSH reads at all, then sampling resumes when shown.
    #[tokio::test(start_paused = true)]
    async fn idles_while_the_header_is_hidden() {
        struct CountingShell(std::sync::atomic::AtomicUsize);
        impl RemoteShell for CountingShell {
            async fn run_sh(&self, _script: &str) -> io::Result<Output> {
                self.0.fetch_add(1, Ordering::Relaxed);
                proc_read()
            }
        }

        let shell = Arc::new(CountingShell(Default::default()));
        let visible = Arc::new(AtomicBool::new(false));
        let (tx, mut rx) = mpsc::channel(16);
        let worker = tokio::spawn({
            // Arc<CountingShell> isn't RemoteShell; forward through a newtype
            struct Shared(Arc<CountingShell>);
            impl RemoteShell for Shared {
                async fn run_sh(&self, script: &str) -> io::Result<Output> {
                    self.0.run_sh(script).await
                }
            }
            remote_system_stats_worker(
                "h".to_string(),
                Shared(Arc::clone(&shell)),
                tx,
                Arc::clone(&visible),
            )
        });

        tokio::time::sleep(SAMPLE_INTERVAL * 10).await;
        assert_eq!(shell.0.load(Ordering::Relaxed), 0, "read while hidden");

        visible.store(true, Ordering::Relaxed);
        let first = rx.recv().await;
        assert!(matches!(first, Some(AppEvent::RemoteSystemStats(..))));
        worker.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_on_a_host_without_proc() {
        let shell = FakeShell::new(vec![exited_with_stderr(
            1,
            "",
            "cat: /proc/stat: No such file or directory",
        )]);
        let (received, _) = run_worker(shell, 1).await;
        assert_eq!(received, 0, "the worker should have stopped");
    }

    #[tokio::test(start_paused = true)]
    async fn survives_a_transient_failed_read() {
        let shell = FakeShell::new(vec![exited(1, ""), exited(0, "garbage"), proc_read()]);
        let (received, finished) = run_worker(shell, 1).await;
        assert_eq!(received, 1);
        assert!(!finished);
    }

    /// Failed reads that aren't a missing /proc (a killed `cat`, say) never
    /// cost the host its line, however many there are.
    #[tokio::test(start_paused = true)]
    async fn never_gives_up_on_other_failed_reads() {
        let mut results: Vec<_> = (0..20).map(|_| exited(137, "")).collect();
        results.push(proc_read());
        let (received, finished) = run_worker(FakeShell::new(results), 1).await;
        assert_eq!(received, 1);
        assert!(!finished);
    }

    #[test]
    fn retry_delay_backs_off_and_caps() {
        let secs = |n| retry_delay(n).as_secs();
        assert_eq!(
            [secs(1), secs(2), secs(3), secs(4), secs(50)],
            [10, 20, 40, 60, 60]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn never_gives_up_on_connection_errors() {
        let mut results: Vec<_> = (0..20).map(|_| connection_error()).collect();
        results.push(proc_read());
        let (received, finished) = run_worker(FakeShell::new(results), 1).await;
        assert_eq!(received, 1);
        assert!(!finished);
    }

    #[test]
    fn rejects_a_short_cpu_line() {
        let short = output("100 0 50 800 50 0 0 0 0 0", 1, 1)
            .replace("cpu  100 0 50 800 50 0 0 0 0 0", "cpu  100 0 50");
        assert_eq!(Snapshot::parse(&short), None);
    }

    #[test]
    fn tolerates_a_truncated_net_dev() {
        // Cut mid-way through the eth0 counters: the partial line is skipped
        // rather than misread, and everything else still parses
        let full = output("100 0 50 800 50 0 0 0 0 0", 1000, 2000);
        let truncated = &full[..full.find("eth0: 1000 10").unwrap() + "eth0: 1000 10".len()];

        let snapshot = Snapshot::parse(truncated).unwrap();
        assert_eq!((snapshot.net_rx_bytes, snapshot.net_tx_bytes), (0, 0));
        assert_eq!(snapshot.cpus, 2);
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
