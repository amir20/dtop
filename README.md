> [!WARNING]
> This project is still under active development and is not ready for use yet.


# dtop

A terminal-based dashboard for monitoring Docker containers in real-time.

## Overview

dtop provides a comprehensive summary of all Docker containers running on your system, displayed directly in your terminal. Get instant visibility into container status, resource usage, and key metrics without leaving the command line.

## Features

- **Real-time monitoring** - Live updates of container status and metrics
- **Comprehensive container information** - View names, IDs, status, ports, and resource usage
- **Clean terminal interface** - Easy-to-read tabular display
- **Lightweight** - Minimal resource footprint
- **No external dependencies** - Works with standard Docker installation

## Installation

```bash
curl -sSfL https://amir20.github.io/dtop/install.sh | bash
```

## Usage

Simply run dtop to see all container information:

```bash
dtop
```

### Screenshot

![dtop screenshot](https://github.com/amir20/dtop/blob/master/demo.gif)

## Options

- `--help` - Display help information
- `--hosts` - A comma separated list of hosts to connect. Defaults to `local`

## Supported Connections

- **Local Docker** - Monitor containers running on the local Docker daemon using `--hosts local`
- **Remote Docker** - Monitor containers running on remote Docker daemons via SSH using `--hosts tcp://host2:2375`
- **SSH Tunneling** - Establish an SSH tunnel to a remote host and monitor containers running on it using `--hosts ssh://user@host`

You can connect to multiple hosts by separating them with commas:

```bash
dtop --hosts local,tcp://host2:2375,ssh://user@host
```

## Requirements

- Docker installed and running
- Terminal with basic color support

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see LICENSE file for details.
