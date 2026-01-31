mod cli;
mod core;
mod docker;
mod ui;

use clap::Parser;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use cli::config::Config;
use cli::connect::{establish_connections, spawn_remaining_connections_handler};
use core::app_state::AppState;
use core::types::{AppEvent, RenderAction, SortField};
use docker::connection::{DockerHost, container_manager};
use ui::icons::IconStyle;
use ui::input::keyboard_worker;
use ui::render::{UiStyles, render_ui};

/// Configuration for the event loop
struct EventLoopConfig {
    icon_style: IconStyle,
    show_all: bool,
    sort_field: SortField,
}

/// Returns custom styles for CLI help output
fn get_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .usage(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .literal(AnsiColor::BrightBlue.on_default())
        .placeholder(AnsiColor::Yellow.on_default())
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Green.on_default())
        .invalid(AnsiColor::Red.on_default())
}

/// Docker container monitoring TUI
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
    long_about = None,
    styles = get_styles()
)]
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

    /// Icon style to use for the UI
    ///
    /// Options:
    ///   unicode  - Standard Unicode icons (default, works everywhere)
    ///   nerd     - Nerd Font icons (requires Nerd Font installed)
    #[arg(short = 'i', long, verbatim_doc_comment)]
    icons: Option<String>,

    /// Filter containers (can be specified multiple times)
    ///
    /// Examples:
    ///   --filter status=running
    ///   --filter name=nginx
    ///   --filter label=com.example.version=1.0
    ///   --filter ancestor=ubuntu:24.04
    ///
    /// Multiple filters of the same type use OR logic:
    ///   --filter status=running --filter status=paused
    ///
    /// Different filter types use AND logic:
    ///   --filter status=running --filter name=nginx
    ///
    /// Available filters:
    ///   id, name, label, status, ancestor, before, since,
    ///   volume, network, publish, expose, health, exited
    ///
    /// Note: Some filters only work with container listing, not events.
    /// Warnings will be shown if a filter is incompatible with events.
    #[arg(short = 'f', long = "filter", verbatim_doc_comment)]
    filter: Vec<String>,

    /// Show all containers (default shows only running containers)
    ///
    /// By default, dtop only shows running containers.
    /// Use this flag to show all containers including stopped, exited, and paused containers.
    ///
    /// Note: This flag can only enable showing all containers, not disable it.
    /// If your config file has 'all: true', you'll need to edit the config file
    /// or press 'a' in the UI to toggle back to showing only running containers.
    ///
    /// This is equivalent to pressing 'a' in the UI to toggle show all.
    #[arg(short = 'a', long = "all", verbatim_doc_comment)]
    all: bool,

    /// Default sort field for container list
    ///
    /// Options:
    ///   uptime  - Sort by container uptime/creation time (default, newest first)
    ///   name    - Sort by container name (alphabetically)
    ///   cpu     - Sort by CPU usage (highest first)
    ///   memory  - Sort by memory usage (highest first)
    ///
    /// You can also use short forms: u, n, c, m
    ///
    /// The sort direction can be toggled in the UI by pressing the same key again.
    #[arg(short = 's', long = "sort", verbatim_doc_comment)]
    sort: Option<String>,
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
        config.merge_with_cli_hosts(
            args.host.clone(),
            false,
            args.filter.clone(),
            args.all,
            args.sort.clone(),
        )
    } else if !config.hosts.is_empty() {
        // No CLI args but config has hosts, use config
        if let Some(path) = config_path {
            eprintln!("Loaded config from: {}", path.display());
        }
        config.merge_with_cli_hosts(
            vec!["local".to_string()],
            true,
            args.filter.clone(),
            args.all,
            args.sort.clone(),
        )
    } else {
        // Neither CLI nor config provided hosts, use default "local"
        config.merge_with_cli_hosts(
            vec!["local".to_string()],
            true,
            args.filter.clone(),
            args.all,
            args.sort.clone(),
        )
    };

    // Determine icon style (CLI takes precedence over config)
    let icon_style = if let Some(ref cli_icons) = args.icons {
        // CLI explicitly provided
        cli_icons.parse::<IconStyle>().unwrap_or_default()
    } else if let Some(ref config_icons) = merged_config.icons {
        // Use config file value
        config_icons.parse::<IconStyle>().unwrap_or_default()
    } else {
        // Default to unicode
        IconStyle::Unicode
    };

    // Determine show_all setting (CLI or config, defaults to false)
    let show_all = merged_config.all.unwrap_or(false);

    // Determine sort field (CLI or config, defaults to Uptime)
    let sort_field = merged_config
        .sort
        .as_ref()
        .and_then(|s| s.parse::<SortField>().ok())
        .unwrap_or(SortField::Uptime);

    // Create event channel
    let (tx, mut rx) = mpsc::channel::<AppEvent>(1000);

    // Establish connections to all configured hosts
    let connection_result = establish_connections(&merged_config, tx.clone()).await?;

    // Store first connected host
    let mut connected_hosts: HashMap<String, DockerHost> = HashMap::new();
    connected_hosts.insert(
        connection_result.first_host.host_id.clone(),
        connection_result.first_host.clone(),
    );

    // Start container manager for first host
    spawn_container_manager(connection_result.first_host, tx.clone());

    // Handle remaining connections in background
    spawn_remaining_connections_handler(connection_result.remaining_rx, tx.clone());

    // Create pause flag for keyboard worker
    let keyboard_paused = Arc::new(AtomicBool::new(false));

    // Spawn keyboard worker in blocking thread
    spawn_keyboard_worker(tx.clone(), keyboard_paused.clone());

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Run main event loop
    run_event_loop(
        &mut terminal,
        &mut rx,
        tx.clone(),
        connected_hosts,
        keyboard_paused,
        EventLoopConfig {
            icon_style,
            show_all,
            sort_field,
        },
    )
    .await?;

    // Restore terminal
    cleanup_terminal(&mut terminal)?;

    Ok(())
}

/// Sets up the terminal for TUI rendering
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

/// Restores the terminal to its original state
fn cleanup_terminal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
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
fn spawn_keyboard_worker(tx: mpsc::Sender<AppEvent>, paused: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        keyboard_worker(tx, paused);
    });
}

/// Main event loop that processes events and renders the UI
async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    rx: &mut mpsc::Receiver<AppEvent>,
    tx: mpsc::Sender<AppEvent>,
    connected_hosts: HashMap<String, DockerHost>,
    keyboard_paused: Arc<AtomicBool>,
    config: EventLoopConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut state = AppState::new(connected_hosts, tx, config.show_all, config.sort_field);
    let draw_interval = Duration::from_millis(500); // Refresh UI every 500ms
    let mut last_draw = std::time::Instant::now();

    // Pre-allocate styles to avoid recreation every frame
    let styles = UiStyles::with_icon_style(config.icon_style);

    while !state.should_quit {
        // Wait for events with timeout - handles both throttling and waiting
        let action = process_events(rx, &mut state, draw_interval).await;

        match action {
            RenderAction::StartShell(container_key) => {
                // Handle shell request - this takes over the terminal
                if let Some(host) = state.connected_hosts.get(&container_key.host_id) {
                    // Pause keyboard worker during shell session
                    keyboard_paused.store(true, Ordering::Relaxed);

                    // Run shell session - this blocks until shell exits
                    if let Err(e) = host.run_shell_session(&container_key.container_id).await {
                        tracing::error!("Shell session error: {}", e);
                    }

                    // Resume keyboard worker
                    keyboard_paused.store(false, Ordering::Relaxed);

                    // Force full redraw after returning from shell
                    terminal.clear()?;
                    terminal.draw(|f| {
                        render_ui(f, &mut state, &styles);
                    })?;
                    last_draw = std::time::Instant::now();
                }
            }
            RenderAction::Render => {
                // Force draw requested
                terminal.draw(|f| {
                    render_ui(f, &mut state, &styles);
                })?;
                last_draw = std::time::Instant::now();
            }
            RenderAction::None => {
                // Check if we should draw based on interval
                if last_draw.elapsed() >= draw_interval {
                    terminal.draw(|f| {
                        render_ui(f, &mut state, &styles);
                    })?;
                    last_draw = std::time::Instant::now();
                }
            }
        }
    }

    Ok(())
}

/// Processes all pending events from the event channel
/// Waits with timeout for at least one event, then drains all pending events
/// Returns the action to take after processing events
async fn process_events(
    rx: &mut mpsc::Receiver<AppEvent>,
    state: &mut AppState,
    timeout: Duration,
) -> RenderAction {
    // Wait for first event with timeout
    let mut result = match tokio::time::timeout(timeout, rx.recv()).await {
        Ok(Some(event)) => state.handle_event(event),
        Ok(None) => {
            // Channel closed
            state.should_quit = true;
            return RenderAction::None;
        }
        Err(_) => {
            // Timeout - no events
            return RenderAction::None;
        }
    };

    // If we got a shell request, return immediately
    if matches!(result, RenderAction::StartShell(_)) {
        return result;
    }

    // Drain any additional pending events without blocking
    while let Ok(event) = rx.try_recv() {
        let action = state.handle_event(event);

        // StartShell takes priority
        if matches!(action, RenderAction::StartShell(_)) {
            return action;
        }

        // Render takes priority over None
        if matches!(action, RenderAction::Render) {
            result = RenderAction::Render;
        }
    }

    result
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
