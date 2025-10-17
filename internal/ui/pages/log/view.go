package log

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
)

func (m Model) View() string {
	var content string
	if m.containerName != "" {
		content = fmt.Sprintf("Viewing logs for: %s\nContainer ID: %s\n\nPress ESC to go back to list", m.containerName, m.containerID)
	} else {
		content = "No container selected\n\nPress ESC to go back to list"
	}
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}
