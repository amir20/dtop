package log

import (
	"context"

	"github.com/amir20/dtop/internal/docker"

	tea "github.com/charmbracelet/bubbletea"
)

func NewModel(ctx context.Context, client *docker.Client) Model {
	return Model{
		ctx:    ctx,
		client: client,
	}
}

// SetContainer updates the model with the container to view logs for
func (m Model) SetContainer(container *docker.Container) Model {
	m.container = container
	return m
}

func (m Model) Init() tea.Cmd {
	return nil
}
