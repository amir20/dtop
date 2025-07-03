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

![dtop screenshot](https://github.com/amir20/dtop/blob/master/demo.png)

## Options

- `--refresh <seconds>` - Set refresh interval (default: 2 seconds)
- `--help` - Display help information
- `--version` - Show version information

## Requirements

- Docker installed and running
- Terminal with basic color support

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see LICENSE file for details.
