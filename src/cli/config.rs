use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for a single Docker host
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HostConfig {
    /// Docker host connection string (e.g., "local", "ssh://user@host")
    pub host: String,

    /// Optional Dozzle URL for this host
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dozzle: Option<String>,
    // Future fields can be added here as optional fields
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub custom_name: Option<String>,
}

/// Configuration that can be loaded from a YAML file
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    /// Docker host(s) to connect to
    #[serde(default)]
    pub hosts: Vec<HostConfig>,
}

impl Config {
    /// Find and load config file from the following locations (in priority order):
    /// 1. ./config.yaml or ./config.yml (relative to current directory)
    /// 2. ~/.config/dtop/config.yaml or ~/.config/dtop/config.yml
    /// 3. ~/.dtop.yaml or ~/.dtop.yml
    ///
    /// Returns (Config, Option<PathBuf>) where the PathBuf is Some if a config file was found
    pub fn load_with_path() -> Result<(Self, Option<PathBuf>), Box<dyn std::error::Error>> {
        let config_paths = Self::get_config_paths();

        for path in config_paths {
            if path.exists() {
                let contents = std::fs::read_to_string(&path)?;
                let config: Config = serde_yaml::from_str(&contents)?;
                return Ok((config, Some(path)));
            }
        }

        Ok((Config::default(), None))
    }

    /// Get list of potential config file paths in priority order
    fn get_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. Relative paths (current directory)
        paths.push(PathBuf::from("config.yaml"));
        paths.push(PathBuf::from("config.yml"));
        paths.push(PathBuf::from(".dtop.yaml"));
        paths.push(PathBuf::from(".dtop.yml"));

        // 2. ~/.config/dtop/config.{yaml,yml}
        if let Some(home) = dirs::home_dir() {
            let config_dir = home.join(".config").join("dtop");
            paths.push(config_dir.join("config.yaml"));
            paths.push(config_dir.join("config.yml"));

            // 3. ~/.dtop.{yaml,yml}
            paths.push(home.join(".dtop.yaml"));
            paths.push(home.join(".dtop.yml"));
        }

        paths
    }

    /// Merge config with command line arguments
    /// CLI args take precedence over config file values
    pub fn merge_with_cli_hosts(mut self, cli_hosts: Vec<String>, cli_default: bool) -> Self {
        // Use CLI hosts if explicitly provided, OR if config file is empty
        if !cli_default || self.hosts.is_empty() {
            // Convert CLI strings to HostConfig structs (no dozzle URL from CLI)
            self.hosts = cli_hosts
                .into_iter()
                .map(|host| HostConfig { host, dozzle: None })
                .collect();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.hosts.is_empty());
    }

    #[test]
    fn test_merge_with_cli_hosts_uses_cli_when_provided() {
        let config = Config {
            hosts: vec![HostConfig {
                host: "ssh://user@server1".to_string(),
                dozzle: None,
            }],
        };

        let merged = config.merge_with_cli_hosts(vec!["ssh://user@server2".to_string()], false);
        assert_eq!(merged.hosts.len(), 1);
        assert_eq!(merged.hosts[0].host, "ssh://user@server2");
    }

    #[test]
    fn test_merge_with_cli_hosts_uses_config_when_cli_is_default() {
        let config = Config {
            hosts: vec![HostConfig {
                host: "ssh://user@server1".to_string(),
                dozzle: Some("https://dozzle.example.com".to_string()),
            }],
        };

        let merged = config.merge_with_cli_hosts(vec!["local".to_string()], true);
        assert_eq!(merged.hosts.len(), 1);
        assert_eq!(merged.hosts[0].host, "ssh://user@server1");
        // Config file's dozzle URL is preserved
        assert_eq!(
            merged.hosts[0].dozzle.as_deref(),
            Some("https://dozzle.example.com")
        );
    }

    #[test]
    fn test_merge_with_cli_hosts_defaults_to_local() {
        let config = Config { hosts: vec![] };

        let merged = config.merge_with_cli_hosts(vec!["local".to_string()], true);
        assert_eq!(merged.hosts.len(), 1);
        assert_eq!(merged.hosts[0].host, "local");
    }

    #[test]
    fn test_yaml_deserialization() {
        let yaml = r#"
hosts:
  - host: local
  - host: ssh://user@server1
  - host: ssh://user@server2:2222
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.hosts.len(), 3);
        assert_eq!(config.hosts[0].host, "local");
        assert_eq!(config.hosts[1].host, "ssh://user@server1");
        assert_eq!(config.hosts[2].host, "ssh://user@server2:2222");
        assert_eq!(config.hosts[0].dozzle, None);
    }

    #[test]
    fn test_yaml_deserialization_with_dozzle() {
        let yaml = r#"
hosts:
  - host: ssh://root@146.190.3.114
    dozzle: https://l.dozzle.dev/
  - host: local
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.hosts.len(), 2);
        assert_eq!(config.hosts[0].host, "ssh://root@146.190.3.114");
        assert_eq!(config.hosts[1].host, "local");
        assert_eq!(
            config.hosts[0].dozzle.as_deref(),
            Some("https://l.dozzle.dev/")
        );
        assert_eq!(config.hosts[1].dozzle, None);
    }

    #[test]
    fn test_host_config_without_dozzle() {
        let host = HostConfig {
            host: "local".to_string(),
            dozzle: None,
        };
        assert_eq!(host.host, "local");
        assert_eq!(host.dozzle, None);
    }

    #[test]
    fn test_host_config_with_dozzle() {
        let host = HostConfig {
            host: "ssh://user@host".to_string(),
            dozzle: Some("https://dozzle.example.com".to_string()),
        };
        assert_eq!(host.host, "ssh://user@host");
        assert_eq!(host.dozzle.as_deref(), Some("https://dozzle.example.com"));
    }
}
