package log

import (
	"github.com/charmbracelet/lipgloss"
)

func (m Model) View() string {
	return m.viewport.View()
}

// StatusBar implements the StatusBar interface
func (m Model) StatusBar() string {
	return lipgloss.PlaceHorizontal(m.width, lipgloss.Center, "Press ESC/← to go back | Press q to quit")
}
