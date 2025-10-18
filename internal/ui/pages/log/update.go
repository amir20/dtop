package log

import (
	"fmt"

	"github.com/amir20/dtop/internal/docker"
	"github.com/amir20/dtop/internal/ui/styles"
	tea "github.com/charmbracelet/bubbletea"
)

func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd

	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		m.viewport.Width = m.width
		m.viewport.Height = m.height

	case docker.LogEntry:
		// Check if we're at the bottom BEFORE adding new content
		wasAtBottom := m.viewport.AtBottom()

		if m.content.Len() > 0 {
			m.content.WriteString("\n")
		}

		// Format timestamp and add colored timestamp prefix
		timestamp := msg.Timestamp.Format("15:04:05.000")
		coloredTimestamp := styles.SelectedStyle.Render(timestamp)
		m.content.WriteString(fmt.Sprintf("%s %s", coloredTimestamp, msg.Message))

		m.viewport.SetContent(m.content.String())

		if wasAtBottom {
			m.viewport.GotoBottom()
		}

		return m, waitForLogs(m.logChannel)
	}

	m.viewport, cmd = m.viewport.Update(msg)
	return m, cmd
}
