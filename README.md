# dtop

A terminal based dashboard for Docker that monitors multiple hosts in real-time.

![dtop screenshot](https://github.com/amir20/dtop/blob/master/demo.gif)

## Overview

`dtop` provides a comprehensive summary of all Docker containers running on your system, displayed directly in your terminal. Get instant visibility into container status, resource usage, and key metrics without leaving the command line. It supports ssh, tcp and local connections and integrates with [Dozzle](https://github.com/amir20/dozzle) for container logs.

## Features

- **Real-time monitoring** - Live updates of container status and metrics
- **Lightweight** - Minimal resource footprint
- **Hyperlinks** - Clickable links to container logs and stats using Dozzle

## Roadmap

- [ ] Add support for disk IO.
- [ ] Add support for Kubernetes clusters
- [ ] Created detailed view, but not to compete with Dozzle
- [ ] Search or filter for containers
- [x] Sort containers by name and status
- [ ] Configurable columns and saving preferences

## Installation
`dtop` can be installed through multiple package managers or by downloading the binary directly.

### Homebrew (macOS and Linux)

This is recommended for macOS and Linux users. Automatic updates are handled by Homebrew.

```sh
brew install --cask amir20/homebrew-dtop/dtop
```

### Docker
`dtop` is released as a Docker image. You can pull it from Docker Hub.

```sh
docker run -v /var/run/docker.sock:/var/run/docker.sock -it amir20/dtop
```

Currently, the image is available for amd64 and arm64 architectures.

### Scoop (Windows)

`dtop` supports prebuilt binaries for Windows. You can install it using [Scoop](https://scoop.sh/).

```sh
scoop bucket add amir20 https://github.com/amir20/scoop-dtop
scoop install amir20/dtop
```

### Install Script

Downloads the latest release from GitHub.

```sh
curl -sSfL https://amir20.github.io/dtop/install.sh | bash
```

### Install using Go

Downloads the latest release from source with Go.

```sh
go install github.com/amir20/dtop@latest
```

## Command Line Options

By default, `dtop` will connect to the local Docker daemon using `/var/run/docker.sock`. `DOCKER_HOST` is also supported to connect to other hosts.

- `--help` - Display help information
- `--hosts` - A comma separated list of hosts to connect. Defaults to `local`


## Configuration File

`dtop` supports command line flags or configuration file. The configuration file reads from the following locations:

- `./config.yaml`
- `~/.dtop.yaml`
- `~/.config/dtop/config.yaml`

> [!Note]
> Both `yaml` and `yml` files are supported.

Here's an example configuration:

```yaml
hosts:
  - host: local
    dozzle: http://localhost:3100/ # this is optional
  - host: tcp://host2:2375
    dozzle: http://host2:3100/
  - host: ssh://user@host
    dozzle: http://host:8080/
```

## Supported Connections

- **Local Docker** - Monitor containers running on the local Docker daemon using `--hosts local`
- **Remote Docker** - Monitor containers running on remote Docker daemons via SSH using `--hosts tcp://host2:2375`
- **SSH Tunneling** - Establish an SSH tunnel to a remote host and monitor containers running on it using `--hosts ssh://user@host`

You can connect to multiple hosts by separating them with commas:

```bash
dtop --hosts local,tcp://host2:2375,ssh://user@host
```

## Dozzle Integration

`dtop` supports linking to container logs using Dozzle. To enable this feature, specify the Dozzle URL in the configuration file or command line flags. Once enabled, `dtop` will automatically open the Dozzle UI when you click on a container. `dtop` leverages [OSC8](https://github.com/Alhadis/OSC8-Adoption/) to send the URL to the terminal. iTerm, Ghostty and a few other terminals supports this with `cmd+click` or `ctrl+click` on the container name. For tmux, you need to have `tmux` version 3.4 or higher installed with `hyperlinks` enabled. This is usually enabled with `set -as terminal-features ",*:hyperlinks"`.

> [!Note]
> Currently, Dozzle url can only be configured in the configuration file. There is no way to provide it directly in the command line flags.

## Related Projects & Inspirations

I am a big fan of [ctop](https://github.com/bcicen/ctop). `ctop` inspired me to create Dozzle but in the browser. However, it seems like `ctop` is no longer maintained. I considered forking `ctop` but deploying with same name would be challenging. I created `dtop` for my personal use case. I often want to see all my containers at a glance across multiple hosts. `dtop` achieves that by supporting remote hosts via `ssh` or `tcp`. Additionally, since I use Dozzle, I integrated Dozzle into `dtop` to provide a seamless experience for monitoring container logs.


## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see LICENSE file for details.
