package main

import (
	"context"
	"fmt"
	"io"
	"log"
	"os"
	"strings"

	"github.com/amir20/dtop/config"
	"github.com/amir20/dtop/internal/docker"
	"github.com/amir20/dtop/internal/ui"

	"github.com/alecthomas/kong"
	kongyaml "github.com/alecthomas/kong-yaml"
	tea "github.com/charmbracelet/bubbletea"
)

var (
	version = "dev"
	commit  = "n/a"
	date    = "n/a"
)

func main() {
	log.SetOutput(io.Discard)
	if _, ok := os.LookupEnv("DEBUG"); ok {
		f, err := os.OpenFile("debug.log", os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o600) //nolint:mnd
		if err != nil {
			fmt.Println("fatal:", err)
			os.Exit(1)
		}
		log.SetOutput(f)
		defer f.Close()
	}
	var cfg config.Cli
	kong.Parse(&cfg, kong.Configuration(kongyaml.Loader, "./config.yaml", "./config.yml", "~/.dtop.yaml", "~/.dtop.yml", "~/.config/dtop/config.yaml", "~/.config/dtop/config.yml"))

	if cfg.Version {
		fmt.Printf("dtop version: %s\nCommit: %s\nBuilt on: %s\n", version, commit, date)
		os.Exit(0)
	}

	var hosts []docker.Host
	for _, hc := range cfg.Hosts {
		if hc.Host == "local" {
			cli, err := config.NewLocalClient()
			if err != nil {
				fmt.Println("Error:", err)
				os.Exit(1)
			}
			host := docker.Host{
				Client:     cli,
				HostConfig: hc,
			}
			hosts = append(hosts, host)
		} else if strings.HasPrefix(hc.Host, "ssh://") {
			cli, err := config.NewSSHClient(hc.Host)
			if err != nil {
				fmt.Println("Error:", err)
				os.Exit(1)
			}
			host := docker.Host{
				Client:     cli,
				HostConfig: hc,
			}
			hosts = append(hosts, host)
		} else if strings.HasPrefix(hc.Host, "tcp://") {
			cli, err := config.NewRemoteClient(hc.Host)
			if err != nil {
				fmt.Println("Error:", err)
				os.Exit(1)
			}
			host := docker.Host{
				Client:     cli,
				HostConfig: hc,
			}
			hosts = append(hosts, host)
		} else {
			fmt.Println("Unsupported host type:", hc.Host)
			os.Exit(1)
		}
	}

	client, err := docker.NewMultiClient(hosts...)
	if err != nil {
		fmt.Println("Error while creating docker client:", err)
		os.Exit(1)
	}

	p := tea.NewProgram(ui.NewModel(context.Background(), client, cfg.Sort), tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		fmt.Println("Error:", err)
		os.Exit(1)
	}
}
