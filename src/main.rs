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

    // Create a channel for receiving successful connections
    let (conn_tx, mut conn_rx) = mpsc::channel::<DockerHost>(merged_config.hosts.len());

    // Collect at least one successful connection before proceeding
    let total_hosts = merged_config.hosts.len();

    // Spawn all connection attempts in parallel
    let connection_handles: Vec<_> = merged_config
        .hosts
        .iter()
        .map(|host_config| {
            let host_config = host_config.clone();
            let conn_tx = conn_tx.clone();
            let error_tx = tx.clone();

            tokio::spawn(async move {
                match connect_and_verify_host(&host_config).await {
                    Ok(docker_host) => {
                        let _ = conn_tx.send(docker_host).await;
                    }
                    Err(e) => {
                        use tracing::error;
                        error!("{}", e);

                        // Create host_id for the error event
                        let host_id = create_host_id(&host_config.host);

                        // Send error event to UI
                        let _ = error_tx
                            .send(AppEvent::ConnectionError(host_id, e.clone()))
                            .await;

                        if total_hosts == 1 {
                            eprintln!("Failed to connect to Docker host: {:?}", e);
                        }
                    }
                }
            })
        })
        .collect();

    // Drop the original sender so the channel closes when all tasks complete
    drop(conn_tx);

    // Try to get the first connection with a reasonable timeout
    match tokio::time::timeout(Duration::from_secs(30), conn_rx.recv()).await {
        Ok(Some(docker_host)) => {
            use tracing::debug;

            // Got first connection! Start the container manager and setup terminal
            connected_hosts.insert(docker_host.host_id.clone(), docker_host.clone());
            spawn_container_manager(docker_host, tx.clone());

            if total_hosts > 1 {
                debug!("Connected to host 1/{}, starting UI...", total_hosts);
            }

            // Continue collecting remaining connections in the background after UI starts
            let remaining_tx = tx.clone();
            tokio::spawn(async move {
                use tracing::debug;
                let mut remaining_count = 1; // Already got one
                while let Some(docker_host) = conn_rx.recv().await {
                    spawn_container_manager(docker_host, remaining_tx.clone());
                    remaining_count += 1;
                    if total_hosts > 1 {
                        debug!("Connected to host {}/{}", remaining_count, total_hosts);
                    }
                }

                // Wait for all connection attempts to complete
                for handle in connection_handles {
                    let _ = handle.await;
                }
            });
        }
        Ok(None) => {
            // Channel closed without any connections
            return Err("Failed to connect to any Docker hosts. Please check your configuration and connection settings. Set DEBUG=1 to see detailed logs in debug.log".into());
        }
        Err(_) => {
            // Timeout waiting for first connection
            return Err("Timeout waiting for Docker host connections (30s). Please check your network and Docker daemon status.".into());
        }
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
/// Returns Ok(DockerHost) if successful, Err with details if connection fails
async fn connect_and_verify_host(
    host_config: &cli::config::HostConfig,
) -> Result<DockerHost, String> {
    use tracing::debug;

    let host_spec = &host_config.host;

    debug!("Attempting to connect to host: {}", host_spec);

    // Attempt to connect
    let docker = connect_docker(host_spec).map_err(|e| {
        format!(
            "Failed to create Docker client for host '{}': {}",
            host_spec, e
        )
    })?;

    debug!("Successfully created Docker client for host: {}", host_spec);

    // Create host ID and DockerHost instance
    let host_id = create_host_id(host_spec);
    let docker_host = DockerHost::new(host_id, docker, host_config.dozzle.clone());

    // Verify the connection actually works by pinging Docker with timeout
    debug!("Pinging Docker daemon at host: {}", host_spec);
    let ping_timeout = Duration::from_secs(10);

    match tokio::time::timeout(ping_timeout, docker_host.docker.ping()).await {
        Ok(Ok(_)) => {
            debug!("Successfully pinged Docker daemon at host: {}", host_spec);
            Ok(docker_host)
        }
        Ok(Err(e)) => {
            debug!("Ping error details: {:?}", e);
            debug!("Error source chain:");
            for (level, err) in std::iter::successors(std::error::Error::source(&e), |e| {
                std::error::Error::source(*e)
            })
            .enumerate()
            {
                debug!("  Level {}: {}", level + 1, err);
            }
            Err(format!(
                "Docker daemon ping failed for host '{}': {}",
                host_spec, e
            ))
        }
        Err(_) => Err(format!(
            "Docker daemon ping timeout for host '{}' (>10s)",
            host_spec
        )),
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
            .with_env_filter(
                EnvFilter::builder()
                    .with_default_directive("dtop=debug".parse()?)
                    .from_env_lossy(),
            )
            .with_ansi(false)
            .init();
    }

    Ok(())
}
