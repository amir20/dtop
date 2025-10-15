# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`dtop` is a terminal-based dashboard for monitoring Docker containers across multiple hosts in real-time. It's written in Go and uses the Bubble Tea TUI framework for the terminal interface. The tool supports local Docker, remote TCP connections, and SSH connections to monitor containers on different hosts.

## Build and Development Commands

### Building
```bash
go build
```

The build uses ldflags to inject version information:
```bash
go build -ldflags="-s -w -X main.version=dev -X main.commit=n/a -X main.date=n/a"
```

### Testing
```bash
go test ./...
```

Run tests for a specific package:
```bash
go test ./internal/docker
go test ./internal/ui
```

### Running
```bash
./dtop
```

With custom hosts:
```bash
./dtop --hosts local,tcp://host2:2375,ssh://user@host
```

With debug logging (creates `debug.log` and `trace.out` files):
```bash
DEBUG=1 ./dtop
```

### Release Build
This project uses GoReleaser for releases:
```bash
goreleaser release --snapshot --clean
```

## Architecture

### Main Entry Point (`main.go`)
- Parses CLI configuration using Kong with YAML support
- Reads configuration from multiple locations: `./config.yaml`, `~/.dtop.yaml`, `~/.config/dtop/config.yaml`
- Creates Docker clients based on host configuration (local, SSH, TCP)
- Initializes the Bubble Tea TUI program with the UI model

### Configuration Layer (`config/`)
- **`config.go`**: Defines CLI structure, host configuration, and sort options
- **`client.go`**: Factory functions for creating Docker clients:
  - `NewLocalClient()`: Connects to local Docker daemon via `/var/run/docker.sock`
  - `NewRemoteClient()`: Connects to remote Docker via TCP with TLS support from `DOCKER_CERT_PATH`
  - `NewSSHClient()`: Establishes SSH tunnel to remote Docker daemon

### Docker Layer (`internal/docker/`)
- **`client.go`**:
  - `MultiClient`: Manages multiple Docker host connections simultaneously
  - `WatchContainers()`: Streams container state changes using Docker events API
  - `WatchContainerStats()`: Streams real-time container statistics (CPU, memory, network)
  - Each host runs in its own goroutine, sending updates through channels
- **`types.go`**: Defines `Container` and `ContainerStat` data structures
- **`calculations.go`**: CPU and memory percentage calculations for Unix and Windows

### UI Layer (`internal/ui/`)
Built with Bubble Tea (elm-style TUI framework):
- **`model.go`**:
  - Main UI model initialization
  - Table configuration with custom column renderers
  - Hyperlink generation using OSC8 for Dozzle integration
- **`update.go`**: Handles Bubble Tea update messages (keypresses, container updates, stats updates)
- **`view.go`**: Renders the UI layout
- **`types.go`**: UI-specific types including keyboard shortcuts
- **`styles.go`**: Lipgloss styling definitions
- **`components/table/`**: Custom table component with sortable columns

### Data Flow
1. Main creates Docker clients for each host
2. `MultiClient.WatchContainers()` starts goroutines that stream container events from Docker API
3. `MultiClient.WatchContainerStats()` starts goroutines that stream live stats for running containers
4. UI model receives updates via Bubble Tea channels
5. Table rows are updated and re-rendered on each update
6. User interactions (sorting, navigation) trigger UI updates through Bubble Tea's update loop

## Key Design Patterns

### Multi-Host Architecture
Each host connection runs in its own goroutine. All hosts send updates to shared channels that the UI consumes. This allows monitoring multiple Docker daemons simultaneously without blocking.

### Event-Driven Updates
The application watches Docker events (start, stop, die) rather than polling. When events occur, it inspects the affected container and sends updates through channels.

### Bubble Tea Architecture
Follows elm-style architecture:
- **Model**: Current state (`model` struct in `ui/model.go`)
- **Update**: Message handlers that return new model state (`ui/update.go`)
- **View**: Pure rendering function (`ui/view.go`)

### Hyperlink Integration
Uses OSC8 terminal escape sequences to embed clickable links to Dozzle (container log viewer). Works with iTerm, Ghostty, and tmux 3.4+.

## Important Implementation Notes

- Container IDs are truncated to 12 characters throughout the codebase
- Stats streaming starts a goroutine per running container
- TLS certificates must be in `DOCKER_CERT_PATH` directory as `ca.pem`, `cert.pem`, `key.pem`
- Debug mode is enabled by setting `DEBUG` environment variable, which creates `debug.log` and `trace.out` files
- The application disables the "open" keybinding when running in an SSH session (detected via `SSH_CLIENT`, `SSH_TTY`, `SSH_CONNECTION` env vars)
- Sort order is preserved in the model and can be toggled between name and status
