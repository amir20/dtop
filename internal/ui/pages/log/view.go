package log

import (
	"github.com/charmbracelet/lipgloss"
)

func (m Model) View() string {
	content := "Logs page - coming soon"
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}
