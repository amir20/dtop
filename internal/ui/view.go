package ui

import "github.com/charmbracelet/lipgloss"

func (m model) View() string {
	return lipgloss.JoinVertical(
		lipgloss.Left, m.table.View(),
		lipgloss.PlaceHorizontal(m.width, lipgloss.Center, helpBarStyle.Render(m.help.View(m.keyMap))),
	)
}
