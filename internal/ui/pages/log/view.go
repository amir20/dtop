package log

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
)

func (m Model) View() string {
	var content string
	if m.container != nil && m.container.Name != "" {
		content = fmt.Sprintf("Viewing logs for: %s\nContainer ID: %s", m.container.Name, m.container.ID)
	} else {
		content = "No container selected"
	}

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

// StatusBar implements the StatusBar interface
func (m Model) StatusBar() string {
	return lipgloss.PlaceHorizontal(m.width, lipgloss.Center, "Press ESC/left to go back | Press q to quit")
}
