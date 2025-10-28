# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Docker Monitor is a terminal-based Docker container monitoring tool built with Rust. It provides real-time CPU and memory metrics for Docker containers through a TUI interface, with support for both local and remote (SSH) Docker daemons. The tool supports **monitoring multiple Docker hosts simultaneously**.

## Build & Run Commands

```bash
# Development
cargo run                                    # Run with local Docker daemon (or config file)
cargo run -- --host ssh://user@host         # Run with remote Docker host via SSH
cargo run -- --host tcp://host:2375         # Run with remote Docker host via TCP
cargo run -- --host local --host ssh://user@host1 --host tcp://host2:2375  # Multiple hosts

# Production build
cargo build --release

# The binary will be at target/release/docker-monitor
```

## Configuration

The application supports configuration via YAML files. Config files are searched in the following order (first found wins):

1. `./config.yaml` or `./config.yml` (relative to current directory)
2. `~/.config/dtui/config.yaml` or `~/.config/dtui/config.yml`
3. `~/dtui.yaml` or `~/dtui.yml`

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
   - Maintains container state in `HashMap<ContainerKey, Container>` where `ContainerKey` is `(host_id, container_id)`
   - Renders UI at 500ms intervals using Ratatui
   - Single source of truth for container data across all hosts

2. **Container Manager** (`docker.rs::container_manager`) - **One per Docker host**
   - Async task that manages Docker API interactions for a specific host
   - Each manager operates independently with its own `DockerHost` instance
   - Fetches initial container list on startup
   - Subscribes to Docker events (start/stop/die) for that host
   - Spawns individual stats stream tasks per container
   - Each container gets its own async task running `stream_container_stats`
   - All events include the `host_id` to identify their source

3. **Keyboard Worker** (`input.rs::keyboard_worker`)
   - Blocking thread that polls keyboard input every 200ms
   - Sends `AppEvent::Quit` on 'q' press
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
- `Quit` - User pressed 'q'
- `Resize` - Terminal was resized
- `SelectPrevious` - Move selection up
- `SelectNext` - Move selection down

### Docker Host Abstraction

The `DockerHost` struct (`docker.rs`) encapsulates a Docker connection with its identifier:

```rust
pub struct DockerHost {
    pub host_id: HostId,
    pub docker: Docker,
}
```

Host IDs are derived from the host specification:
- `"local"` → host_id = `"local"`
- `"ssh://user@host"` → host_id = `"user@host"`
- `"ssh://user@host:2222"` → host_id = `"user@host"` (port stripped)

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

CPU and memory percentages are calculated in `docker.rs`:
- **CPU**: Delta between current and previous CPU usage, normalized by system CPU delta and CPU count
- **Memory**: Current usage divided by limit, expressed as percentage

### UI Rendering

The UI (`ui.rs`) uses pre-allocated styles to avoid per-frame allocations. The table now includes a "Host" column to identify which Docker host each container belongs to.

Color coding:
- Green: 0-50%
- Yellow: 50-80%
- Red: >80%

Containers are sorted first by `host_id`, then by container name within each host.

## CI/CD Workflows

### Release Workflow (`.github/workflows/release.yml`)
- Triggers on version tags (e.g., `v0.1.0`)
- Builds for 4 platforms using matrix strategy:
  - Linux x86_64 (native cargo)
  - Linux ARM64 (cross tool)
  - macOS x86_64 (native cargo on macOS runner)
  - macOS ARM64 (native cargo on macOS runner)
- Uses `softprops/action-gh-release@v2` to create GitHub releases

### PR Build Workflow (`.github/workflows/pr-build.yml`)
- Same build matrix as release workflow
- Posts comment on PR with artifact download links
- Updates existing comment on subsequent pushes

**Note**: `cross` is only used for Linux ARM64. macOS builds require native runners because Docker can't containerize macOS.

## Key Dependencies

- **Tokio**: Async runtime for Docker API and event handling
- **Bollard**: Docker API client with SSH support
- **Ratatui**: Terminal UI framework
- **Crossterm**: Cross-platform terminal manipulation
- **Clap**: CLI argument parsing
- **Serde/Serde_yaml**: Configuration file deserialization
- **Dirs**: Cross-platform home directory detection

## Performance Considerations

- UI refresh rate is throttled to 500ms to reduce CPU usage
- Container stats streams run independently per container across all hosts
- Each host's container manager runs independently without blocking other hosts
- Keyboard polling is 200ms to balance responsiveness and CPU
- Styles are pre-allocated in `UiStyles::default()` to avoid allocations during rendering
- Container references (not clones) are used when building UI rows
- Failed host connections are logged but don't prevent other hosts from being monitored
