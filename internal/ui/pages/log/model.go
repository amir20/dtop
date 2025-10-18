package log

import (
	"context"

	"github.com/amir20/dtop/internal/docker"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
)

func NewModel(ctx context.Context, client *docker.Client, container *docker.Container, width int, height int) Model {
	newCtx, cancel := context.WithCancel(ctx)
	logChannel, err := client.StreamLogs(newCtx, container)

	if err != nil {
		panic(err)
	}

	return Model{
		ctx:        newCtx,
		client:     client,
		container:  container,
		cancel:     cancel,
		width:      width,
		height:     height,
		viewport:   viewport.New(width, height),
		logChannel: logChannel,
	}
}

func (m Model) Init() tea.Cmd {
	return waitForLogs(m.logChannel)
}

func waitForLogs(ch <-chan docker.LogEntry) tea.Cmd {
	return func() tea.Msg {
		entry := <-ch
		return entry
	}
}

// Destroy implements the Destroy interface
func (m Model) Destroy() {
	m.cancel()
}
