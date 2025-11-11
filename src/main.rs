mod cli;
mod core;
mod docker;
mod ui;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;
use url::Url;

use cli::config::Config;
use core::app_state::AppState;
use core::types::AppEvent;
use docker::connection::{DockerHost, connect_docker, container_manager};
use ui::input::keyboard_worker;
use ui::render::{UiStyles, render_ui};

/// Docker container monitoring TUI
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Docker host(s) to connect to. Can be specified multiple times.
    ///
    /// Examples:
    ///   --host local                    (Connect to local Docker daemon)
    ///   --host ssh://user@host          (Connect via SSH)
    ///   --host ssh://user@host:2222     (Connect via SSH with custom port)
    ///   --host tcp://host:2375          (Connect via TCP to remote Docker daemon)
    ///   --host tls://host:2376          (Connect via TLS)
    ///   --host local --host ssh://user@server1 --host tls://server2:2376  (Multiple hosts)
    ///
    /// For TLS connections, set DOCKER_CERT_PATH to a directory containing:
    ///   key.pem, cert.pem, and ca.pem
    ///
    /// If not specified, will use config file or default to "local"
    #[arg(short = 'H', long, verbatim_doc_comment)]
    host: Vec<String>,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Update dtop to the latest version
    #[cfg(feature = "self-update")]
    Update,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    setup_logging()?;

    // Parse command line arguments
    let args = Args::parse();

    // Handle subcommands before initializing Tokio runtime
    if let Some(command) = args.command {
        match command {
            #[cfg(feature = "self-update")]
            Command::Update => {
                return cli::update::run_update();
            }
        }
    }

    // Run the main TUI in async context
    run_async(args)
}

#[tokio::main]
async fn run_async(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // Determine if CLI hosts were explicitly provided
    let cli_provided = !args.host.is_empty();

    // Load config file only if CLI hosts not provided
    let (config, config_path) = if cli_provided {
        // User explicitly provided --host, don't load config for hosts
        (Config::default(), None)
    } else {
        // Load config file if it exists
        Config::load_with_path()?
    };

    // Merge config with CLI args (CLI takes precedence)
    let merged_config = if cli_provided {
        // User explicitly provided --host, use CLI args
        config.merge_with_cli_hosts(args.host.clone(), false)
    } else if !config.hosts.is_empty() {
        // No CLI args but config has hosts, use config
        if let Some(path) = config_path {
            eprintln!("Loaded config from: {}", path.display());
        }
        config
    } else {
        // Neither CLI nor config provided hosts, use default "local"
        config.merge_with_cli_hosts(vec!["local".to_string()], true)
    };

    // Create event channel
    let (tx, mut rx) = mpsc::channel::<AppEvent>(1000);

    // Store DockerHost instances for log streaming
    let mut connected_hosts: HashMap<String, DockerHost> = HashMap::new();

    // Connect to all hosts and spawn container managers
    for host_config in &merged_config.hosts {
        if let Some(docker_host) =
            connect_and_verify_host(&host_config.host, host_config.dozzle.clone()).await
        {
            connected_hosts.insert(docker_host.host_id.clone(), docker_host.clone());
            spawn_container_manager(docker_host, tx.clone());
        }
    }

    // Check if at least one host connected successfully
    if connected_hosts.is_empty() {
        return Err("Failed to connect to any Docker hosts. Please check your configuration and connection settings.".into());
    }

    // Spawn keyboard worker in blocking thread
    spawn_keyboard_worker(tx.clone());

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Run main event loop
    run_event_loop(&mut terminal, &mut rx, tx.clone(), connected_hosts).await?;

    // Restore terminal
    cleanup_terminal(&mut terminal)?;

    Ok(())
}

/// Connects to a Docker host and verifies the connection works
/// Returns Some(DockerHost) if successful, None if connection fails
async fn connect_and_verify_host(
    host_spec: &str,
    dozzle_url: Option<String>,
) -> Option<DockerHost> {
    use tracing::{debug, error};

    let is_ssh = host_spec.starts_with("ssh://");

    debug!("Attempting to connect to host: {}", host_spec);

    // Attempt to connect
    let docker = match connect_docker(host_spec) {
        Ok(docker) => {
            debug!("Successfully created Docker client for host: {}", host_spec);
            if is_ssh {
                debug!("SSH transport layer established");
            }
            docker
        }
        Err(e) => {
            error!(
                "Failed to create Docker client for host '{}': {}",
                host_spec, e
            );
            debug!("Error details: {:?}", e);

            if is_ssh {
                let host_part = host_spec.strip_prefix("ssh://").unwrap_or(host_spec);
                eprintln!("Failed to connect to host '{}': {}", host_spec, e);
                eprintln!("\nDebug steps to diagnose SSH connection:");
                eprintln!(
                    "  1. Test SSH access:       ssh {} 'echo SSH works'",
                    host_part
                );
                eprintln!("  2. Test Docker on remote: ssh {} 'docker ps'", host_part);
                eprintln!(
                    "  3. Check Docker socket:   ssh {} 'ls -la /var/run/docker.sock'",
                    host_part
                );
                eprintln!("  4. Check user groups:     ssh {} 'groups'", host_part);
                eprintln!(
                    "  5. Check Docker daemon:   ssh {} 'systemctl status docker'",
                    host_part
                );
                eprintln!(
                    "\nFor detailed logs, run with: DEBUG=1 dtop --host {}",
                    host_spec
                );
            } else {
                eprintln!("Failed to connect to host '{}': {}", host_spec, e);
            }
            return None;
        }
    };

    // Create host ID and DockerHost instance
    let host_id = create_host_id(host_spec);
    let docker_host = DockerHost::new(host_id, docker, dozzle_url);

    // Verify the connection actually works by pinging Docker with timeout
    debug!("Pinging Docker daemon at host: {}", host_spec);
    let ping_timeout = Duration::from_secs(10);
    match tokio::time::timeout(ping_timeout, docker_host.docker.ping()).await {
        Ok(Ok(_)) => {
            debug!("Successfully pinged Docker daemon at host: {}", host_spec);
            Some(docker_host)
        }
        Ok(Err(e)) => {
            error!("Docker daemon ping failed for host '{}': {}", host_spec, e);
            eprintln!("Failed to connect to host '{}': {}", host_spec, e);
            debug!("Ping error details: {:?}", e);
            debug!("Error source chain:");
            let mut source = std::error::Error::source(&e);
            let mut level = 1;
            while let Some(err) = source {
                debug!("  Level {}: {}", level, err);
                source = std::error::Error::source(err);
                level += 1;
            }

            eprintln!("Failed to connect to host '{}': {}", host_spec, e);

            if is_ssh {
                let host_part = host_spec.strip_prefix("ssh://").unwrap_or(host_spec);
                eprintln!("\nSSH connection established but Docker API call failed.");
                eprintln!("Common causes:");
                eprintln!(
                    "  • Docker daemon not running:  ssh {} 'systemctl status docker'",
                    host_part
                );
                eprintln!(
                    "  • Permission denied:          ssh {} 'docker ps'",
                    host_part
                );
                eprintln!(
                    "  • User not in docker group:   ssh {} 'groups' (should show 'docker')",
                    host_part
                );
                eprintln!(
                    "  • Socket permissions:         ssh {} 'stat /var/run/docker.sock'",
                    host_part
                );
                eprintln!(
                    "\nIf 'docker ps' works over SSH but dtop fails, please file a bug report."
                );
                eprintln!(
                    "Enable detailed logs with: DEBUG=1 dtop --host {}",
                    host_spec
                );
            }
            None
        }
        Err(_) => {
            error!("Docker daemon ping timeout for host: {}", host_spec);
            eprintln!(
                "Failed to connect to host '{}': Docker ping timeout (>10s)",
                host_spec
            );

            if is_ssh {
                eprintln!("\nTimeout suggests slow connection or unresponsive Docker daemon");
            }
            None
        }
    }
}

/// Creates a unique host identifier from the host specification
fn create_host_id(host_spec: &str) -> String {
    if host_spec == "local" {
        "local".to_string()
    } else if let Ok(url) = Url::parse(host_spec) {
        // Extract just the domain/host from the URL
        url.host_str().unwrap_or(host_spec).to_string()
    } else {
        host_spec.to_string()
    }
}

/// Sets up the terminal for TUI rendering
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

/// Restores the terminal to its original state
fn cleanup_terminal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Spawns the container manager task for a specific host
fn spawn_container_manager(docker_host: DockerHost, tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        container_manager(docker_host, tx).await;
    });
}

/// Spawns the keyboard input worker thread
fn spawn_keyboard_worker(tx: mpsc::Sender<AppEvent>) {
    std::thread::spawn(move || {
        keyboard_worker(tx);
    });
}

/// Main event loop that processes events and renders the UI
async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    rx: &mut mpsc::Receiver<AppEvent>,
    tx: mpsc::Sender<AppEvent>,
    connected_hosts: HashMap<String, DockerHost>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut state = AppState::new(connected_hosts, tx);
    let draw_interval = Duration::from_millis(500); // Refresh UI every 500ms
    let mut last_draw = std::time::Instant::now();

    // Pre-allocate styles to avoid recreation every frame
    let styles = UiStyles::default();

    while !state.should_quit {
        // Wait for events with timeout - handles both throttling and waiting
        let force_draw = process_events(rx, &mut state, draw_interval).await;

        // Draw UI if forced (table structure changed) or if draw_interval has elapsed
        let should_draw = force_draw || last_draw.elapsed() >= draw_interval;

        if should_draw {
            terminal.draw(|f| {
                render_ui(f, &mut state, &styles);
            })?;
            last_draw = std::time::Instant::now();
        }
    }

    Ok(())
}

/// Processes all pending events from the event channel
/// Waits with timeout for at least one event, then drains all pending events
/// Returns true if a force draw is needed (table structure changed)
async fn process_events(
    rx: &mut mpsc::Receiver<AppEvent>,
    state: &mut AppState,
    timeout: Duration,
) -> bool {
    let mut force_draw = false;

    // Wait for first event with timeout
    match tokio::time::timeout(timeout, rx.recv()).await {
        Ok(Some(event)) => {
            force_draw |= state.handle_event(event);
        }
        Ok(None) => {
            // Channel closed
            state.should_quit = true;
            return false;
        }
        Err(_) => {
            // Timeout - no events, just return without forcing draw
            return false;
        }
    }

    // Drain any additional pending events without blocking
    while let Ok(event) = rx.try_recv() {
        force_draw |= state.handle_event(event);
    }

    force_draw
}

fn setup_logging() -> Result<(), Box<dyn std::error::Error>> {
    // Check if DEBUG is enabled
    if std::env::var("DEBUG").is_ok() {
        let log_file = File::create("debug.log")?;

        tracing_subscriber::fmt()
            .with_writer(log_file)
            .with_env_filter(EnvFilter::new("dtop=debug"))
            .with_ansi(false)
            .init();
    }

    Ok(())
}
