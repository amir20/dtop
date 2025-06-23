package main

import (
	"context"
	"fmt"
	"os"
	"term-test/internal/docker"
	"term-test/internal/ui"

	tea "github.com/charmbracelet/bubbletea"
)

func main() {
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
