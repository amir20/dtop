//! SSH transport for the Docker API with support for custom ports.
//!
//! Bollard's built-in SSH connector (`Docker::connect_with_ssh`) hands the URI
//! authority (`user@host:2222`) straight to `openssh::SessionBuilder::resolve`,
//! which only splits off the port when the string still carries the `ssh://`
//! scheme. Without the scheme the whole `host:port` is treated as a hostname and
//! `ssh` fails with `Could not resolve hostname host:port`.
//!
//! This module re-implements the connector (based on bollard's `src/ssh.rs`,
//! MIT licensed) and resolves the destination with the scheme intact so
//! `ssh://user@host:2222` works. It is wired up through
//! `Docker::connect_with_custom_transport`.
//!
//! Connections are multiplexed over a small pool of shared SSH masters. dtop
//! holds one long-lived HTTP connection per container (the stats stream), plus
//! the event stream and any open log stream, so launching a master per
//! connection meant one TCP connection and one auth handshake per container —
//! 31 of them for 30 containers. Each master here carries up to
//! [`MAX_CHANNELS_PER_MASTER`] channels instead, which keeps us comfortably
//! under sshd's default `MaxSessions` of 10 while cutting connection count by
//! ~8x.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, ready};
use std::time::Duration;

use bollard::errors::Error as BollardError;
use bollard::{ClientVersion, Docker};
use futures_util::FutureExt;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

/// Channels opened per SSH master. sshd's default `MaxSessions` is 10; staying
/// under it leaves headroom for a session opened outside dtop.
const MAX_CHANNELS_PER_MASTER: usize = 8;

/// How often the master pings the server, so a dead master surfaces as a spawn
/// error (and gets evicted) instead of hanging.
const SERVER_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

/// Connects to a Docker daemon over SSH, honoring an optional port in `addr`.
///
/// `addr` is expected in the `ssh://[user@]host[:port]` form.
pub fn connect_with_ssh(
    addr: &str,
    timeout: u64,
    client_version: &ClientVersion,
) -> Result<Docker, BollardError> {
    let connector = SshConnector {
        pool: Arc::new(Pool::new(MAX_CHANNELS_PER_MASTER)),
    };
    let client = Arc::new(
        hyper_util::client::legacy::Client::builder(TokioExecutor::new()).build(connector),
    );

    Docker::connect_with_custom_transport(
        move |req: bollard::BollardRequest| {
            let client = Arc::clone(&client);
            Box::pin(async move { client.request(req).await.map_err(BollardError::from) })
        },
        Some(addr),
        timeout,
        client_version,
    )
}

/// A pooled SSH master and its remaining channel capacity.
struct Master<S> {
    id: usize,
    session: S,
    slots: Arc<Semaphore>,
}

/// One channel's claim on a master. Dropping it returns the slot to the pool,
/// so capacity is tied to the lifetime of the connection that holds it.
struct Lease<S> {
    session: S,
    master_id: usize,
    _permit: OwnedSemaphorePermit,
}

/// Hands out channel slots on shared sessions, launching a new one only when
/// every existing session is at capacity.
///
/// Generic over the session type so the capacity accounting can be tested
/// without launching SSH; production code uses `Pool<Arc<openssh::Session>>`.
struct Pool<S> {
    masters: Mutex<Vec<Master<S>>>,
    next_id: AtomicUsize,
    max_channels: usize,
}

impl<S: Clone> Pool<S> {
    fn new(max_channels: usize) -> Self {
        Self {
            masters: Mutex::new(Vec::new()),
            next_id: AtomicUsize::new(0),
            max_channels,
        }
    }

    /// Returns a lease on a master with spare capacity, calling `launch` to
    /// create one only if all existing masters are full.
    ///
    /// The lock is deliberately held across `launch`: it serializes master
    /// creation so a burst of connections at startup doesn't open a pile of
    /// simultaneous handshakes.
    async fn acquire<F, Fut, E>(&self, launch: F) -> Result<Lease<S>, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<S, E>>,
    {
        let mut masters = self.masters.lock().await;

        for master in masters.iter() {
            if let Ok(permit) = Arc::clone(&master.slots).try_acquire_owned() {
                return Ok(Lease {
                    session: master.session.clone(),
                    master_id: master.id,
                    _permit: permit,
                });
            }
        }

        let session = launch().await?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let slots = Arc::new(Semaphore::new(self.max_channels));
        let permit = Arc::clone(&slots)
            .try_acquire_owned()
            .expect("a fresh semaphore always has a permit available");

        masters.push(Master {
            id,
            session: session.clone(),
            slots,
        });

        Ok(Lease {
            session,
            master_id: id,
            _permit: permit,
        })
    }

    /// Drops a master from the pool so the next acquire launches a fresh one.
    /// Channels already running on it keep it alive until they finish.
    async fn evict(&self, id: usize) {
        self.masters.lock().await.retain(|master| master.id != id);
    }

    #[cfg(test)]
    async fn len(&self) -> usize {
        self.masters.lock().await.len()
    }
}

type SshPool = Pool<Arc<openssh::Session>>;

#[derive(Clone)]
struct SshConnector {
    pool: Arc<SshPool>,
}

/// Launches a control master for `destination`, which keeps the `ssh://` scheme
/// so openssh splits off the user and port instead of passing `host:port` to
/// ssh as a hostname.
async fn launch_master(destination: &str) -> Result<Arc<openssh::Session>, openssh::Error> {
    let mut builder = openssh::SessionBuilder::default();
    builder.server_alive_interval(SERVER_ALIVE_INTERVAL);

    let (builder, destination) = builder.resolve(destination);
    let tempdir = builder.launch_master(destination).await?;

    Ok(Arc::new(openssh::Session::new_process_mux(tempdir)))
}

/// Opens a `docker system dial-stdio` channel on an existing master.
async fn dial_stdio(session: &Arc<openssh::Session>) -> Result<SshChild, openssh::Error> {
    let mut child = Arc::clone(session)
        .arc_command("docker")
        .arg("system")
        .arg("dial-stdio")
        .stdin(openssh::Stdio::piped())
        .stdout(openssh::Stdio::piped())
        .spawn()
        .await?;

    let stdin = TokioIo::new(child.stdin().take().expect("stdin was piped"));
    let stdout = TokioIo::new(child.stdout().take().expect("stdout was piped"));

    Ok(SshChild {
        child,
        stdin,
        stdout,
    })
}

struct SshChild {
    child: openssh::Child<Arc<openssh::Session>>,
    stdin: TokioIo<openssh::ChildStdin>,
    stdout: TokioIo<openssh::ChildStdout>,
}

struct SshStream {
    _child: openssh::Child<Arc<openssh::Session>>,
    /// Held for the life of the connection; frees a channel slot on drop.
    _lease: OwnedSemaphorePermit,
    stdin: Option<TokioIo<openssh::ChildStdin>>,
    stdout: TokioIo<openssh::ChildStdout>,
}

impl tower_service::Service<hyper::Uri> for SshConnector {
    type Response = SshStream;
    type Error = openssh::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, uri: hyper::Uri) -> Self::Future {
        let pool = Arc::clone(&self.pool);

        async move {
            let destination = ssh_destination(&uri).map_err(openssh::Error::Connect)?;

            // A pooled master can die between being launched and being reused
            // (network drop, daemon restart, ControlPersist timeout). Evict it
            // and retry once on a freshly launched one.
            let mut last_err = None;

            for _ in 0..2 {
                let lease = pool.acquire(|| launch_master(&destination)).await?;

                match dial_stdio(&lease.session).await {
                    Ok(child) => {
                        return Ok(SshStream {
                            _child: child.child,
                            _lease: lease._permit,
                            stdin: Some(child.stdin),
                            stdout: child.stdout,
                        });
                    }
                    Err(err) => {
                        tracing::debug!(
                            "SSH master {} failed to open a channel, evicting it: {}",
                            lease.master_id,
                            err
                        );
                        pool.evict(lease.master_id).await;
                        last_err = Some(err);
                    }
                }
            }

            Err(last_err.expect("loop records an error before exiting"))
        }
        .boxed()
    }
}

/// Builds the `ssh://user@host:port` destination string from a request URI.
fn ssh_destination(uri: &hyper::Uri) -> io::Result<String> {
    match uri.scheme_str() {
        Some("ssh") => {}
        scheme => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid scheme {scheme:?}"),
            ));
        }
    }

    let authority = uri
        .authority()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing authority"))?;

    Ok(format!("ssh://{authority}"))
}

impl SshStream {
    fn stdin(self: Pin<&mut Self>) -> io::Result<Pin<&mut TokioIo<openssh::ChildStdin>>> {
        self.get_mut()
            .stdin
            .as_mut()
            .map(Pin::new)
            .ok_or_else(|| io::ErrorKind::BrokenPipe.into())
    }

    fn stdout(self: Pin<&mut Self>) -> Pin<&mut TokioIo<openssh::ChildStdout>> {
        Pin::new(&mut self.get_mut().stdout)
    }
}

impl hyper::rt::Read for SshStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        self.stdout().poll_read(cx, buf)
    }
}

impl hyper::rt::Write for SshStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.stdin()?.poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.stdin()?.poll_write_vectored(cx, bufs)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.stdin()?.poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.as_mut().stdin()?.poll_shutdown(cx))?;
        // drop stdin to shutdown the input half.
        drop(self.get_mut().stdin.take());
        Poll::Ready(Ok(()))
    }
}

impl hyper_util::client::legacy::connect::Connection for SshStream {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        hyper_util::client::legacy::connect::Connected::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destination_keeps_scheme_user_and_port() {
        let uri: hyper::Uri = "ssh://user@msi.lan:2022/v1.53/containers/json"
            .parse()
            .unwrap();
        assert_eq!(ssh_destination(&uri).unwrap(), "ssh://user@msi.lan:2022");
    }

    #[test]
    fn destination_without_port() {
        let uri: hyper::Uri = "ssh://user@msi.lan/v1.53/_ping".parse().unwrap();
        assert_eq!(ssh_destination(&uri).unwrap(), "ssh://user@msi.lan");
    }

    #[test]
    fn destination_rejects_other_schemes() {
        let uri: hyper::Uri = "http://msi.lan:2375/v1.53/_ping".parse().unwrap();
        assert!(ssh_destination(&uri).is_err());
    }

    /// Launcher that hands out sequential fake sessions and counts launches.
    fn counting_launcher(count: &AtomicUsize) -> impl FnOnce() -> BoxFuture + '_ {
        move || {
            let id = count.fetch_add(1, Ordering::Relaxed);
            Box::pin(async move { Ok::<_, ()>(id) })
        }
    }

    type BoxFuture = Pin<Box<dyn Future<Output = Result<usize, ()>> + Send>>;

    #[tokio::test]
    async fn reuses_one_master_up_to_capacity() {
        let pool = Pool::<usize>::new(3);
        let launches = AtomicUsize::new(0);

        let leases: Vec<_> = {
            let mut leases = Vec::new();
            for _ in 0..3 {
                leases.push(pool.acquire(counting_launcher(&launches)).await.unwrap());
            }
            leases
        };

        assert_eq!(launches.load(Ordering::Relaxed), 1);
        assert_eq!(pool.len().await, 1);
        assert!(leases.iter().all(|lease| lease.session == 0));
    }

    #[tokio::test]
    async fn launches_a_second_master_past_capacity() {
        let pool = Pool::<usize>::new(2);
        let launches = AtomicUsize::new(0);

        let mut leases = Vec::new();
        for _ in 0..5 {
            leases.push(pool.acquire(counting_launcher(&launches)).await.unwrap());
        }

        // 5 channels at 2 per master => 3 masters.
        assert_eq!(launches.load(Ordering::Relaxed), 3);
        assert_eq!(pool.len().await, 3);
    }

    #[tokio::test]
    async fn dropping_a_lease_frees_the_slot() {
        let pool = Pool::<usize>::new(1);
        let launches = AtomicUsize::new(0);

        let lease = pool.acquire(counting_launcher(&launches)).await.unwrap();
        assert_eq!(launches.load(Ordering::Relaxed), 1);

        drop(lease);

        // The freed slot is reused instead of launching another master.
        let _reused = pool.acquire(counting_launcher(&launches)).await.unwrap();
        assert_eq!(launches.load(Ordering::Relaxed), 1);
        assert_eq!(pool.len().await, 1);
    }

    #[tokio::test]
    async fn evicted_master_is_replaced_on_next_acquire() {
        let pool = Pool::<usize>::new(4);
        let launches = AtomicUsize::new(0);

        let lease = pool.acquire(counting_launcher(&launches)).await.unwrap();
        pool.evict(lease.master_id).await;
        assert_eq!(pool.len().await, 0);

        let replacement = pool.acquire(counting_launcher(&launches)).await.unwrap();

        assert_eq!(launches.load(Ordering::Relaxed), 2);
        assert_eq!(replacement.session, 1);
        assert_eq!(pool.len().await, 1);
    }

    #[tokio::test]
    async fn evicting_an_unknown_id_is_a_no_op() {
        let pool = Pool::<usize>::new(4);
        let launches = AtomicUsize::new(0);

        let lease = pool.acquire(counting_launcher(&launches)).await.unwrap();
        pool.evict(lease.master_id + 999).await;

        assert_eq!(pool.len().await, 1);
    }

    #[tokio::test]
    async fn a_failed_launch_leaves_the_pool_empty() {
        let pool = Pool::<usize>::new(4);

        let result = pool
            .acquire(|| Box::pin(async { Err::<usize, _>("boom") }))
            .await;

        assert_eq!(result.err(), Some("boom"));
        assert_eq!(pool.len().await, 0);
    }

    #[tokio::test]
    async fn concurrent_acquires_share_one_master() {
        let pool = Arc::new(Pool::<usize>::new(8));
        let launches = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let pool = Arc::clone(&pool);
                let launches = Arc::clone(&launches);
                tokio::spawn(async move {
                    let lease = pool
                        .acquire(|| {
                            let id = launches.fetch_add(1, Ordering::Relaxed);
                            Box::pin(async move { Ok::<_, ()>(id) })
                        })
                        .await
                        .unwrap();
                    lease.session
                })
            })
            .collect();

        let sessions: Vec<_> = futures_util::future::join_all(handles)
            .await
            .into_iter()
            .map(|joined| joined.unwrap())
            .collect();

        // The lock serializes acquires, so 8 concurrent callers fit on one master.
        assert_eq!(launches.load(Ordering::Relaxed), 1);
        assert!(sessions.iter().all(|session| *session == 0));
    }

    #[test]
    fn openssh_resolves_port_from_destination() {
        let default = openssh::SessionBuilder::default();
        // The builder returned here carries the user/port overrides; ssh is
        // invoked with `-p 2022` and just the hostname as the destination.
        let (_builder, host) = default.resolve("ssh://user@msi.lan:2022");
        assert_eq!(host, "msi.lan");
    }
}
