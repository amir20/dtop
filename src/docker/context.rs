//! Docker CLI context resolution.
//!
//! When connecting to the "local" Docker daemon, dtop mirrors the Docker CLI's
//! endpoint resolution so it works out of the box with tools like colima and
//! Rancher Desktop that expose the daemon on a non-default socket.
//!
//! Resolution order (matching the Docker CLI):
//! 1. `DOCKER_HOST` environment variable (any scheme) — highest priority.
//! 2. `DOCKER_CONTEXT` environment variable naming a context.
//! 3. `currentContext` in `~/.docker/config.json`.
//! 4. The built-in `default` context (the OS default socket).
//!
//! For a named (non-default) context, the endpoint is read from the context
//! metadata stored at `~/.docker/contexts/meta/<sha256(name)>/meta.json`.

use std::path::PathBuf;

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Minimal view of a context `meta.json` file: we only need the Docker endpoint.
#[derive(Debug, Deserialize)]
struct ContextMeta {
    #[serde(rename = "Endpoints")]
    endpoints: Endpoints,
}

#[derive(Debug, Deserialize)]
struct Endpoints {
    docker: Option<DockerEndpoint>,
}

#[derive(Debug, Deserialize)]
struct DockerEndpoint {
    #[serde(rename = "Host")]
    host: Option<String>,
}

/// Minimal view of `~/.docker/config.json`: we only need `currentContext`.
#[derive(Debug, Deserialize)]
struct DockerConfig {
    #[serde(rename = "currentContext")]
    current_context: Option<String>,
}

/// Resolves the endpoint that the "local" host should connect to, following the
/// Docker CLI's resolution order.
///
/// Returns `Some(endpoint)` (e.g. `unix:///Users/me/.colima/default/docker.sock`
/// or `tcp://host:2375`) when a specific endpoint should be used, or `None` when
/// the caller should fall back to the OS default connection method.
pub fn resolve_local_endpoint() -> Option<String> {
    resolve_with(
        std::env::var("DOCKER_HOST").ok(),
        active_context_name(),
        dirs::home_dir(),
    )
}

/// Determines the active context name from `DOCKER_CONTEXT` or `config.json`.
fn active_context_name() -> Option<String> {
    if let Ok(ctx) = std::env::var("DOCKER_CONTEXT")
        && !ctx.is_empty()
    {
        return Some(ctx);
    }
    read_current_context(dirs::home_dir())
}

/// Reads `currentContext` from `~/.docker/config.json`, if present.
fn read_current_context(home: Option<PathBuf>) -> Option<String> {
    let path = home?.join(".docker").join("config.json");
    let contents = std::fs::read_to_string(path).ok()?;
    let config: DockerConfig = serde_json::from_str(&contents).ok()?;
    config
        .current_context
        .filter(|c| !c.is_empty() && c != "default")
}

/// Core resolution logic, split out for testability.
fn resolve_with(
    docker_host: Option<String>,
    context: Option<String>,
    home: Option<PathBuf>,
) -> Option<String> {
    // 1. DOCKER_HOST always wins, matching the Docker CLI.
    if let Some(host) = docker_host
        && !host.is_empty()
    {
        return Some(host);
    }

    // 2/3. A named context resolves to its stored endpoint.
    let context = context?;
    if context.is_empty() || context == "default" {
        return None;
    }

    context_endpoint(&context, home)
}

/// Reads the Docker endpoint for a named context from its `meta.json`.
fn context_endpoint(name: &str, home: Option<PathBuf>) -> Option<String> {
    let meta_path = home?
        .join(".docker")
        .join("contexts")
        .join("meta")
        .join(context_dir_name(name))
        .join("meta.json");

    let contents = std::fs::read_to_string(&meta_path).ok()?;
    let meta: ContextMeta = serde_json::from_str(&contents).ok()?;
    meta.endpoints
        .docker
        .and_then(|d| d.host)
        .filter(|h| !h.is_empty())
}

/// The metadata directory for a context is the lowercase hex SHA-256 of its name.
fn context_dir_name(name: &str) -> String {
    let digest = Sha256::digest(name.as_bytes());
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn dir_name_matches_docker_hash() {
        // Lowercase hex SHA-256 of "colima", matching `docker context inspect`.
        assert_eq!(
            context_dir_name("colima"),
            "f24fd3749c1368328e2b149bec149cb6795619f244c5b584e844961215dadd16"
        );
        assert_eq!(context_dir_name("colima").len(), 64);
        assert!(
            context_dir_name("colima")
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
    }

    #[test]
    fn docker_host_takes_precedence() {
        let result = resolve_with(
            Some("tcp://1.2.3.4:2375".to_string()),
            Some("colima".to_string()),
            None,
        );
        assert_eq!(result, Some("tcp://1.2.3.4:2375".to_string()));
    }

    #[test]
    fn empty_docker_host_is_ignored() {
        // No context and empty DOCKER_HOST => fall back to defaults.
        assert_eq!(resolve_with(Some(String::new()), None, None), None);
    }

    #[test]
    fn default_context_falls_back() {
        assert_eq!(resolve_with(None, Some("default".to_string()), None), None);
    }

    #[test]
    fn no_context_falls_back() {
        assert_eq!(resolve_with(None, None, None), None);
    }

    #[test]
    fn named_context_reads_endpoint_from_meta() {
        let tmp = std::env::temp_dir().join(format!("dtop-ctx-test-{}", std::process::id()));
        let meta_dir = tmp
            .join(".docker")
            .join("contexts")
            .join("meta")
            .join(context_dir_name("colima"));
        fs::create_dir_all(&meta_dir).unwrap();
        fs::write(
            meta_dir.join("meta.json"),
            r#"{"Name":"colima","Endpoints":{"docker":{"Host":"unix:///Users/me/.colima/default/docker.sock"}}}"#,
        )
        .unwrap();

        let result = resolve_with(None, Some("colima".to_string()), Some(tmp.clone()));
        assert_eq!(
            result,
            Some("unix:///Users/me/.colima/default/docker.sock".to_string())
        );

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn missing_context_meta_falls_back() {
        assert_eq!(
            resolve_with(
                None,
                Some("does-not-exist".to_string()),
                Some(PathBuf::from("/nonexistent"))
            ),
            None
        );
    }
}
