package main

import (
	"context"
	"fmt"

	"dtop/internal/docker"
	"dtop/internal/ui"
	"os"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
)

func main() {
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})

	client, err := docker.NewLocalClient()
	if err != nil {
		fmt.Println("Error:", err)
		os.Exit(1)
	}

	p := tea.NewProgram(ui.NewModel(context.Background(), client), tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		fmt.Println("Error:", err)
		os.Exit(1)
	}
}
