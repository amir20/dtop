package log

import (
	"github.com/amir20/dtop/internal/docker"
	tea "github.com/charmbracelet/bubbletea"
)

func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		m.viewport.Width = m.width
		m.viewport.Height = m.height
		return m, nil

	case docker.LogEntry:
		m.viewport.SetContent(m.viewport.View() + "\n" + msg.Message)
		m.viewport.GotoBottom()
		return m, waitForLogs(m.logChannel)
	}

	return m, nil
}
