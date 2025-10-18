package log

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
)

func (m Model) View() string {
	var content string
	if m.container != nil && m.container.Name != "" {
		content = fmt.Sprintf("Viewing logs for: %s\nContainer ID: %s\n\nPress ESC to go back to list\nPress q to quit", m.container.Name, m.container.ID)
	} else {
		content = "No container selected\n\nPress ESC to go back to list\nPress q to quit"
	}
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}
