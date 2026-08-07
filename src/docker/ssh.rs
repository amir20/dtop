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

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, ready};

use bollard::errors::Error as BollardError;
use bollard::{ClientVersion, Docker};
use futures_util::FutureExt;
use hyper_util::rt::{TokioExecutor, TokioIo};

/// Connects to a Docker daemon over SSH, honoring an optional port in `addr`.
///
/// `addr` is expected in the `ssh://[user@]host[:port]` form.
pub fn connect_with_ssh(
    addr: &str,
    timeout: u64,
    client_version: &ClientVersion,
) -> Result<Docker, BollardError> {
    let client = Arc::new(
        hyper_util::client::legacy::Client::builder(TokioExecutor::new()).build(SshConnector),
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

#[derive(Clone)]
struct SshConnector;

struct SshStream {
    _child: openssh::Child<Arc<openssh::Session>>,
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
        async move {
            let destination = ssh_destination(&uri).map_err(openssh::Error::Connect)?;

            // Keep the `ssh://` scheme so openssh splits off the user and port
            // instead of passing `host:port` to ssh as a hostname.
            let builder = openssh::SessionBuilder::default();
            let (builder, destination) = builder.resolve(&destination);
            let tempdir = builder.launch_master(destination).await?;
            let session = Arc::new(openssh::Session::new_process_mux(tempdir));

            let mut child = session
                .arc_command("docker")
                .arg("system")
                .arg("dial-stdio")
                .stdin(openssh::Stdio::piped())
                .stdout(openssh::Stdio::piped())
                .spawn()
                .await?;

            Ok(SshStream {
                stdin: Some(TokioIo::new(child.stdin().take().unwrap())),
                stdout: TokioIo::new(child.stdout().take().unwrap()),
                _child: child,
            })
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

    #[test]
    fn openssh_resolves_port_from_destination() {
        let default = openssh::SessionBuilder::default();
        // The builder returned here carries the user/port overrides; ssh is
        // invoked with `-p 2022` and just the hostname as the destination.
        let (_builder, host) = default.resolve("ssh://user@msi.lan:2022");
        assert_eq!(host, "msi.lan");
    }
}
