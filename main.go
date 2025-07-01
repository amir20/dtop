package main

import (
	"context"
	"fmt"
	"net/http"
	"os"
	"strings"

	"dtop/config"
	"dtop/internal/docker"
	"dtop/internal/ui"

	"github.com/alecthomas/kong"
	kongyaml "github.com/alecthomas/kong-yaml"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/docker/cli/cli/connhelper"
	"github.com/docker/docker/client"
)

var (
	version = "dev"
	commit  = "n/a"
	date    = "n/a"
)

func main() {
	var cfg config.CliConfig
	kong.Parse(&cfg, kong.Configuration(kongyaml.Loader, "./config.yaml", "~/.config/dtop/config.yaml", "~/.dtop.yaml"))

	if cfg.Version {
		fmt.Printf("dtop version: %s\nCommit: %s\nBuilt on: %s\n", version, commit, date)
		os.Exit(0)
	}

	var clients []*client.Client
	for _, host := range cfg.Hosts {
		if host == "local" {
			cli, err := newLocalClient()
			if err != nil {
				fmt.Println("Error:", err)
				os.Exit(1)
			}
			clients = append(clients, cli)
		} else if strings.HasPrefix(host, "ssh://") {
			cli, err := newSSHClient(host)
			if err != nil {
				fmt.Println("Error:", err)
				os.Exit(1)
			}
			clients = append(clients, cli)
		} else if strings.HasPrefix(host, "tcp://") {
			cli, err := newRemoteClient(host)
			if err != nil {
				fmt.Println("Error:", err)
				os.Exit(1)
			}
			clients = append(clients, cli)
		} else {
			fmt.Println("Unsupported host type:", host)
			os.Exit(1)
		}
	}

	client := docker.NewMultiClient(clients...)

	p := tea.NewProgram(ui.NewModel(context.Background(), client), tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		fmt.Println("Error:", err)
		os.Exit(1)
	}
}

func newLocalClient() (*client.Client, error) {
	cli, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation(), client.WithUserAgent("Docker-Client/dtop"))
	if err != nil {
		return nil, err
	}
	return cli, nil
}

func newRemoteClient(host string) (*client.Client, error) {
	cli, err := client.NewClientWithOpts(client.WithHost(host), client.WithAPIVersionNegotiation(), client.WithUserAgent("Docker-Client/dtop"))
	if err != nil {
		return nil, err
	}
	return cli, nil
}

func newSSHClient(host string) (*client.Client, error) {
	helper, err := connhelper.GetConnectionHelper(host)
	if err != nil {
		return nil, err
	}

	httpClient := &http.Client{
		Transport: &http.Transport{
			DialContext: helper.Dialer, // This sets up the tunnel over SSH
		},
	}

	cli, err := client.NewClientWithOpts(
		client.WithHTTPClient(httpClient),
		client.WithHost(helper.Host),
		client.WithDialContext(helper.Dialer),
		client.WithAPIVersionNegotiation(),
	)

	if err != nil {
		return nil, err
	}

	return cli, nil
}
