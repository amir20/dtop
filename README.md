# 🖥️ dtop

> [!IMPORTANT]
> This project is currently being rewritten in Rust on the `master` branch. The Go version is available on the `v1` branch.

A terminal based dashboard for Docker that monitors multiple hosts in real-time.

![dtop screenshot](https://github.com/amir20/dtop/blob/master/demo.gif)

## Overview

`dtop` provides a comprehensive summary of all Docker containers running on your system, displayed directly in your terminal. Get instant visibility into container status, resource usage, and key metrics without leaving the command line. It supports ssh, tcp and local connections and integrates with [Dozzle](https://github.com/amir20/dozzle) for container logs.

## Features

- 💻 **Real-time monitoring** - Live updates of container status and metrics
- ⚡ **Lightweight** - Minimal resource footprint using Rust
- ⌨ **Dozzle** - Supports opening Dozzle links

## Roadmap

- [x] Sort containers by name and status
- [ ] Add support for disk IO.
- [ ] Add support for Kubernetes clusters
- [x] Implement log view streaming (basic)
- [ ] Search or filter for containers
- [ ] Support multiple certs for TLS
- [ ] Configurable columns and saving preferences

## Installation
`dtop` can be installed through multiple package managers or by downloading the binary directly.


### Docker
`dtop` is released as a docker image. You can pull it from Github.

```sh
docker run -v /var/run/docker.sock:/var/run/docker.sock -it ghcr.io/amir20/dtop
```

Currently, the image is available for amd64 and arm64 architectures.

### Install Script

Downloads the latest release from GitHub.

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/amir20/dtop/releases/latest/download/dtop-installer.sh | sh
```

### Install from Source

`dtop` is written in Rust can be installed using Cargo.

```sh
cargo install --git https://github.com/amir20/dtop
```

## Command Line Options

By default, `dtop` will connect to the local Docker daemon using `/var/run/docker.sock`. `DOCKER_HOST` is also supported to connect to other hosts.

A terminal-based Docker container monitoring tool with real-time CPU and memory metrics

    Usage: dtop [OPTIONS]

    Options:
      -H, --host <HOST>
              Docker host(s) to connect to. Can be specified multiple times.

              Examples: --host local                    (Connect to local Docker daemon) --host ssh://user@host          (Connect via SSH) --host ssh://user@host:2222     (Connect via SSH with custom port) --host tcp://host:2375          (Connect via TCP to remote Docker daemon) --host local --host ssh://user@server1 --host tcp://server2:2375  (Multiple hosts)

              If not specified, will use config file or default to "local"

      -h, --help
              Print help (see a summary with '-h')

      -V, --version
              Print version

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

See [config.example.yaml](https://github.com/amir20/dtop/blob/master/config.example.yaml) for more examples.

## Supported Connections

- **Local Docker** - Monitor containers running on the local Docker daemon using `--hosts local`
- **Remote Docker** - Monitor containers running on remote Docker daemons via SSH using `--hosts tcp://host2:2375`
- **SSH** - Establish an SSH connection to a remote host and monitor containers running on it using `--hosts ssh://user@host`

You can connect to multiple hosts by separating them with commas:

```bash
dtop --host local --host tcp://host2:2375 --host ssh://user@host
```
> [!Note]
> Currently, Dozzle url can only be configured in the configuration file. There is no way to provide it directly in the command line flags.

## Related Projects & Inspirations

I am a big fan of [ctop](https://github.com/bcicen/ctop). `ctop` inspired me to create Dozzle but in the browser. However, it seems like `ctop` is no longer maintained. I considered forking `ctop` but deploying with same name would be challenging. I created `dtop` for my personal use case. I often want to see all my containers at a glance across multiple hosts. `dtop` achieves that by supporting remote hosts via `ssh` or `tcp`. Additionally, since I use Dozzle, I integrated Dozzle into `dtop` to provide a seamless experience for monitoring container logs.


## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see LICENSE file for details.
