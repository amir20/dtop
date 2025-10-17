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

func (m Model) Init() tea.Cmd {
	return nil
}
