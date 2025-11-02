# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`dtop` is a terminal-based Docker container monitoring tool built with Rust. It provides real-time CPU, memory, and network metrics for Docker containers through a TUI interface, with support for both local and remote (SSH/TCP) Docker daemons. The tool supports **monitoring multiple Docker hosts simultaneously** and includes a **built-in log viewer** for streaming container logs.

## Build & Run Commands

```bash
# Development
cargo run                                    # Run with local Docker daemon (or config file)
cargo run -- --host ssh://user@host         # Run with remote Docker host via SSH
cargo run -- --host tcp://host:2375         # Run with remote Docker host via TCP
cargo run -- --host local --host ssh://user@host1 --host tcp://host2:2375  # Multiple hosts

# Self-update
cargo run -- update                          # Update dtop to the latest version
dtop update                                  # (or use the installed binary)

# Testing
cargo test                                   # Run all tests
cargo test -- --nocapture                    # Run tests with output
cargo insta test                             # Run tests with snapshot review
cargo insta accept                           # Accept all pending snapshots
cargo insta reject                           # Reject all pending snapshots

# Production build
cargo build --release                        # The binary will be at target/release/dtop (includes self-update)
cargo build --release --no-default-features  # Build without self-update feature (smaller binary)

# Changelog generation
git-cliff --latest                           # Generate changelog for the latest tag
git-cliff --unreleased                       # Generate changelog for unreleased changes
git-cliff --tag v0.3.0..v0.3.6              # Generate changelog for a version range
git-cliff -o CHANGELOG.md                    # Write changelog to file

# Docker build
docker build -t dtop .
docker run -v /var/run/docker.sock:/var/run/docker.sock -it dtop
```

## Configuration

The application supports configuration via YAML files. Config files are searched in the following order (first found wins):

1. `./config.yaml` or `./config.yml` (relative to current directory)
2. `~/.config/dtop/config.yaml` or `~/.config/dtop/config.yml`
3. `~/.dtop.yaml` or `~/.dtop.yml`

**Command line arguments take precedence over config file values.**

Example config file (`config.yaml`):
```yaml
hosts:
  - host: local
  - host: ssh://user@server1
  - host: tcp://192.168.1.100:2375
  - host: ssh://root@146.190.3.114
    dozzle: https://l.dozzle.dev/
```

Each host entry is a struct with:
- `host`: Docker connection string (required)
- `dozzle`: Optional URL to Dozzle instance
- Future optional fields can be added as needed

See `config.example.yaml` for a complete example.

## Architecture

The application follows an **event-driven architecture** with multiple async/threaded components communicating via a single mpsc channel (`AppEvent`). The architecture supports **multi-host monitoring** by spawning independent container managers for each Docker host.

### Core Components

1. **Main Event Loop** (`main.rs::run_event_loop`)
   - Receives events from all container managers via a shared channel
   - Delegates state management to `AppState` struct
   - Renders UI at 500ms intervals using Ratatui
   - Uses throttling to wait for events or timeout, then drains all pending events

2. **AppState** (`app_state.rs::AppState`)
   - Central state manager that handles all runtime data
   - Maintains container state in `HashMap<ContainerKey, Container>` where `ContainerKey` is `(host_id, container_id)`
   - Manages view state (container list vs log view)
   - Handles log streaming, scrolling, and auto-scroll behavior
   - Pre-sorts containers by host_id and name for efficient rendering
   - Single source of truth for container data across all hosts

3. **Container Manager** (`docker.rs::container_manager`) - **One per Docker host**
   - Async task that manages Docker API interactions for a specific host
   - Each manager operates independently with its own `DockerHost` instance
   - Fetches initial container list on startup
   - Subscribes to Docker events (start/stop/die) for that host
   - Spawns individual stats stream tasks per container
   - Each container gets its own async task running `stream_container_stats`
   - All events include the `host_id` to identify their source

4. **Stats Streaming** (`stats.rs::stream_container_stats`)
   - One async task per container that streams real-time stats
   - Uses **exponential moving average (alpha=0.3)** to smooth CPU, memory, and network stats
   - Calculates network TX/RX rates in bytes per second
   - CPU calculation: Delta between current and previous usage, normalized by system CPU delta and CPU count
   - Memory calculation: Current usage divided by limit, expressed as percentage

5. **Log Streaming** (`logs.rs::stream_container_logs`)
   - Streams logs from a container in real-time
   - Fetches last 100 lines on startup, then follows new logs
   - Parses timestamps (RFC3339 format) and messages
   - Uses `ansi-to-tui` to parse ANSI escape codes for colored output
   - Preserves whitespace and formatting from original logs
   - Sends each log line as `AppEvent::LogLine` event with pre-parsed Text

6. **Keyboard Worker** (`input.rs::keyboard_worker`)
   - Blocking thread that polls keyboard input every 200ms
   - Handles: 'q'/Ctrl-C (quit), Enter (view logs), Esc (exit log view), Up/Down (navigate/scroll)
   - Separate thread because crossterm's event polling is blocking

### Multi-Host Architecture

```
Host1 (local)     → container_manager → AppEvent(host_id="local", ...) ┐
Host2 (server1)   → container_manager → AppEvent(host_id="server1", ...)├→ Main Loop → UI
Host3 (server2)   → container_manager → AppEvent(host_id="server2", ...)┘
Keyboard          → keyboard_worker   → AppEvent::Quit → Main Loop → Exit
```

**Key Design Points:**
- Each host runs its own independent `container_manager` task
- All container managers share the same event channel (`mpsc::Sender<AppEvent>`)
- Every event includes a `host_id` to identify which host it came from
- Containers are uniquely identified by `ContainerKey { host_id, container_id }`
- The UI displays host information alongside container information

### Event Types (`types.rs::AppEvent`)

Container-related events use structured types to identify containers across hosts:

- `InitialContainerList(HostId, Vec<Container>)` - Batch of containers from a specific host on startup
- `ContainerCreated(Container)` - New container started (host_id is in the Container struct)
- `ContainerDestroyed(ContainerKey)` - Container stopped/died (identified by host_id + container_id)
- `ContainerStat(ContainerKey, ContainerStats)` - Stats update (identified by host_id + container_id)
- `ContainerHealthChanged(ContainerKey, HealthStatus)` - Health status changed for a container
- `Quit` - User pressed 'q' or Ctrl-C
- `Resize` - Terminal was resized
- `SelectPrevious` - Move selection up (Up arrow in container list)
- `SelectNext` - Move selection down (Down arrow in container list)
- `EnterPressed` - User pressed Enter to view logs
- `ExitLogView` - User pressed Escape to exit log view
- `ScrollUp` - Scroll up in log view (Up arrow)
- `ScrollDown` - Scroll down in log view (Down arrow)
- `LogLine(ContainerKey, LogEntry)` - New log line received from streaming logs
- `OpenDozzle` - User pressed 'o' to open Dozzle for selected container
- `ToggleHelp` - User pressed '?' to toggle help popup
- `CycleSortField` - User pressed 's' to cycle through sort fields
- `SetSortField(SortField)` - User pressed a specific key to set sort field (u/n/c/m)
- `ToggleShowAll` - User pressed 'a' to toggle showing all containers (including stopped)

### View States (`types.rs::ViewState`)

The application has two view states:
- `ContainerList` - Main view showing all containers across all hosts
- `LogView(ContainerKey)` - Log viewer for a specific container with real-time streaming

### Container Data Model (`types.rs::Container`)

The `Container` struct holds both static metadata and runtime statistics:

```rust
pub struct Container {
    pub id: String,                         // Truncated container ID (12 chars)
    pub name: String,                       // Container name
    pub state: ContainerState,              // Running, Paused, Exited, etc.
    pub health: Option<HealthStatus>,       // Healthy, Unhealthy, Starting (None if no health check)
    pub created: Option<DateTime<Utc>>,     // Container creation timestamp
    pub stats: ContainerStats,              // CPU, memory, network stats (updated in real-time)
    pub host_id: HostId,                    // Which Docker host this container belongs to
    pub dozzle_url: Option<String>,         // Dozzle URL for this container's host
}
```

**Container Identification:**
- Containers are uniquely identified by `ContainerKey { host_id, container_id }`
- This allows tracking the same container ID across different hosts
- Container IDs are truncated to 12 characters (Docker API accepts partial IDs)

### Docker Host Abstraction

The `DockerHost` struct (`docker.rs`) encapsulates a Docker connection with its identifier and optional Dozzle URL:

```rust
pub struct DockerHost {
    pub host_id: HostId,
    pub docker: Docker,
    pub dozzle_url: Option<String>,
}
```

Host IDs are derived from the host specification:
- `"local"` → host_id = `"local"`
- `"ssh://user@host"` → host_id = `"user@host"`
- `"ssh://user@host:2222"` → host_id = `"user@host"` (port stripped)

**Dozzle Integration:**
- Dozzle URLs can be configured per-host in the config file
- Press 'o' in container list view to open Dozzle for the selected container
- Opens in browser at `{dozzle_url}/container/{container_id}` format
- Only works when not in an SSH session (detected via SSH_CLIENT/SSH_TTY/SSH_CONNECTION env vars)

### Configuration Loading

The `Config` struct (`config.rs`) handles YAML configuration file loading:
- Searches multiple locations in priority order (see Configuration section above)
- Merges config file values with CLI arguments (CLI takes precedence)
- Uses `serde_yaml` for deserialization
- Uses `dirs` crate for home directory detection

**Host Configuration Format:**
The `HostConfig` struct contains:
- `host`: String - The Docker connection string (required)
- `dozzle`: Option<String> - URL to Dozzle instance (optional)
- Additional optional fields can be added in the future

All fields except `host` are optional and use `#[serde(skip_serializing_if = "Option::is_none")]`.

The merge logic:
- If CLI hosts are explicitly provided (not default), they override config file
- If CLI uses default (`--host local`) and config file has hosts, config file is used
- If both are empty/default, defaults to `local`
- CLI hosts are converted to `HostConfig` structs with `dozzle: None`

### Docker Connection

The `connect_docker()` function in `main.rs` handles three connection modes:
- `--host local`: Uses local Docker socket
- `--host ssh://user@host[:port]`: Connects via SSH (requires Bollard SSH feature)
- `--host tcp://host:port`: Connects via TCP to remote Docker daemon

Multiple `--host` arguments can be provided to monitor multiple Docker hosts simultaneously.

**Note:** TCP connections are unencrypted. Only use on trusted networks or with proper firewall rules.

### Stats Calculation

Stats are calculated in `stats.rs` with exponential smoothing applied:
- **CPU**: Delta between current and previous CPU usage, normalized by system CPU delta and CPU count
- **Memory**: Current usage divided by limit, expressed as percentage
- **Network**: Calculates TX/RX rates by tracking byte deltas over time
- **Smoothing**: Uses exponential moving average with alpha=0.3 to reduce noise and create smoother visualizations

### UI Rendering

The UI (`ui.rs`) uses pre-allocated styles to avoid per-frame allocations.

**Two View Modes:**
1. **Container List View** - Main table showing all containers
   - Dynamically shows/hides "Host" column (only shown when multiple hosts are connected)
   - Displays: ID, Name, Host (conditional), CPU%, Memory%, Net TX, Net RX, Status
   - Progress bars with percentage indicators for CPU and Memory
   - Network rates formatted as B/s, KB/s, MB/s, or GB/s
2. **Log View** - Full-screen log streaming for selected container
   - Shows last 100 lines initially, then follows new logs
   - Timestamps displayed in yellow with bold formatting
   - Auto-scroll when at bottom, manual scroll preserves position
   - Displays "[AUTO]" or "[MANUAL]" indicator in title

**Color Coding for Metrics:**
- Green: 0-50%
- Yellow: 50.1-80%
- Red: >80%

**Sorting:** Containers can be sorted by multiple fields:
- Default sort: Uptime (newest first, descending)
- Sort fields: Uptime, Name, CPU, Memory
- Containers are always sorted by `host_id` first, then by the selected field within each host
- Press 's' to cycle through sort fields
- Press 'u'/'n'/'c'/'m' to sort by specific field (Uptime/Name/CPU/Memory)
- Pressing the same field key toggles sort direction (ascending/descending)
- Each field has a default direction: Name (ascending), Uptime/CPU/Memory (descending)
- Sort state is tracked in `SortState` with field and direction

**Container Filtering:**
- By default, only running containers are shown
- Press 'a' to toggle showing all containers (including stopped/exited containers)
- Filter state is tracked in `AppState::show_all_containers`

**Health Status:**
- Containers with health checks display their status: Healthy, Unhealthy, Starting
- Health status is parsed from Docker's health check information
- Status changes trigger UI updates via `ContainerHealthChanged` event

## CI/CD Workflows

The project uses `cargo-dist` for building and releasing binaries across multiple platforms.

### Release Workflow (`.github/workflows/release.yml`)
- Triggers on version tags (e.g., `v0.1.0`)
- Uses `cargo-dist` for cross-platform builds
- Builds for multiple platforms: Linux (x86_64, ARM64), macOS (x86_64, ARM64)
- Automatically creates GitHub releases with generated changelogs
- Produces installers, archives, and checksums
- Three main jobs:
  - `plan`: Determines what needs to be built
  - `build-local-artifacts`: Builds platform-specific binaries
  - `build-global-artifacts`: Creates installers and checksums
  - `host`: Uploads artifacts and creates GitHub release

### Other Workflows
- `.github/workflows/pr-build.yml` - Builds on pull requests
- `.github/workflows/docker-build.yml` - Builds Docker images for testing
- `.github/workflows/docker-release.yml` - Publishes Docker images on release
- `.github/workflows/test.yml` - Runs test suite

## Build Features

The project supports optional features to control binary size and dependencies:

### `self-update` Feature (enabled by default)
- Adds the `dtop update` subcommand for self-updating the binary
- Depends on `self_update` crate with rustls (adds ~1.9MB to binary size)
- **Included in**: Release binaries, cargo-dist builds, regular cargo builds
- **Excluded from**: Docker images (to minimize image size)

**Usage:**
```bash
# Build with self-update (default)
cargo build --release                        # Binary: ~3.8MB

# Build without self-update (smaller)
cargo build --release --no-default-features  # Binary: ~1.9MB
```

**Docker Configuration:**
The Dockerfile builds with `--no-default-features` to create minimal Docker images (~2.5MB vs ~4.7MB).
Since Docker containers are typically updated by pulling new images, the self-update feature isn't needed.
## Changelog Management

The project uses `git-cliff` for automated changelog generation based on conventional commits.

### Configuration

The changelog is configured in `cliff.toml` with the following settings:
- Follows conventional commit format (feat, fix, docs, chore, etc.)
- Groups commits by type (Features, Bug Fixes, Documentation, etc.)
- Filters out dependency update commits and release preparation commits
- Supports semantic versioning tags (v[0-9]+\.[0-9]+\.[0-9]+)
- Sorts commits within sections by oldest first

### Conventional Commit Format

Commits should follow this format:
```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Common types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `perf`: Performance improvements
- `refactor`: Code refactoring
- `style`: Code style changes
- `test`: Test changes
- `chore`: Maintenance tasks (builds, CI, etc.)

**Examples:**
```
feat(update): add self-update command
fix(stats): correct CPU calculation for multi-core systems
docs: update installation instructions
chore(deps): update rust crate bollard to v0.19.4
```

### Generating Changelogs

```bash
# View latest release changelog
git-cliff --latest

# View unreleased changes
git-cliff --unreleased

# Generate changelog for a version range
git-cliff --tag v0.3.0..v0.3.6

# Write full changelog to file
git-cliff -o CHANGELOG.md

# Generate changelog and update for next version
git-cliff --unreleased --tag v0.4.0 -o CHANGELOG.md
```

### Integration with Cargo

The project integrates git-cliff with both `cargo-release` and `cargo-dist`:

**cargo-dist integration (`dist-workspace.toml`):**
- `generate-changelog = true` - Automatically generates changelogs during releases
- `changelog-backend = "git-cliff"` - Uses git-cliff for changelog generation
- The GitHub release workflow will automatically include the generated changelog

**cargo-release integration (`Cargo.toml`):**
- `pre-release-replacements` - Automatically updates CHANGELOG.md during `cargo release`
- Adds new version entry when creating a release

**Release Workflow:**
1. Make changes and commit using conventional commit format
2. Run `cargo release <version>` to create a new release
3. CHANGELOG.md is automatically updated with the new version
4. Tag is created and pushed to GitHub
5. GitHub Actions (via cargo-dist) builds binaries and creates a release with the changelog

The `CHANGELOG.md` file is automatically maintained and should be committed to the repository.

## Key Dependencies

- **Tokio**: Async runtime for Docker API and event handling
- **Bollard**: Docker API client with SSH support (requires `ssh` feature)
- **Ratatui**: Terminal UI framework (v0.29)
- **Crossterm**: Cross-platform terminal manipulation (v0.29)
- **Clap**: CLI argument parsing with derive macros
- **Serde/Serde_yaml**: Configuration file deserialization
- **Dirs**: Cross-platform home directory detection
- **Chrono**: Date and time handling for log timestamps
- **Futures-util**: Stream utilities for async operations
- **Open**: Cross-platform URL opener for Dozzle integration
- **Ansi-to-tui**: ANSI escape code parsing for colored log output
- **Timeago**: Human-readable time formatting for container uptime

### Dev Dependencies
- **Insta**: Snapshot testing (use `cargo insta accept` to accept snapshots)
- **Mockall**: Mock generation for testing

## Performance Considerations

- UI refresh rate is throttled to 500ms to reduce CPU usage
- Event processing uses timeout-based throttling: waits for first event with timeout, then drains all pending
- Container stats streams run independently per container across all hosts
- Each host's container manager runs independently without blocking other hosts
- Keyboard polling is 200ms to balance responsiveness and CPU
- Styles are pre-allocated in `UiStyles::default()` to avoid allocations during rendering
- Container references (not clones) are used when building UI rows
- Containers are pre-sorted once when added/removed, not on every frame
- Exponential smoothing (alpha=0.3) reduces noise in stats without heavy computation
- Failed host connections are logged but don't prevent other hosts from being monitored
- Log streaming is only active when viewing a container's logs (stopped when exiting log view)
- Log text is formatted once when received and cached in `AppState::formatted_log_text` to avoid re-parsing ANSI codes on every frame
- ANSI parsing happens at log arrival time, not render time

## User Interactions

**Container List View:**
- `↑/↓` - Navigate between containers
- `Enter` - View logs for selected container
- `q` or `Ctrl-C` - Quit application
- `o` - Open Dozzle for selected container (if configured and not in SSH session)
- `?` - Toggle help popup
- `s` - Cycle through sort fields (Uptime → Name → CPU → Memory → Uptime)
- `u` - Sort by Uptime (toggle direction if already sorting by Uptime)
- `n` - Sort by Name (toggle direction if already sorting by Name)
- `c` - Sort by CPU (toggle direction if already sorting by CPU)
- `m` - Sort by Memory (toggle direction if already sorting by Memory)
- `a` - Toggle showing all containers (including stopped containers)

**Log View:**
- `↑/↓` - Scroll through logs manually
- `Esc` - Return to container list
- `?` - Toggle help popup
- Auto-scroll behavior: Automatically scrolls to bottom when new logs arrive (unless manually scrolled up)

## Testing Strategy

The codebase includes unit tests for:
- Stats calculation logic (`stats.rs`): CPU percentage, memory percentage, edge cases
- UI color coding (`ui.rs`): Threshold boundaries for green/yellow/red
- Log parsing (`logs.rs`): Timestamp parsing, message extraction, edge cases
- Config loading (`config.rs`): YAML deserialization, CLI merging, host configurations

Run tests with `cargo test`.
