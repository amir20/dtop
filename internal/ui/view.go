package ui

import "github.com/charmbracelet/lipgloss"

func (m model) View() string {
	keymap := m.keyMap
	rows := m.table.Rows()
	if len(rows) > 0 && keymap.Open.Enabled() && m.table.Cursor() != -1 {
		selected := rows[m.table.Cursor()]
		if selected.container.Dozzle == "" {
			keymap.Open.SetEnabled(false)
		}
	}

	return lipgloss.JoinVertical(
		lipgloss.Left, m.table.View(),
		lipgloss.PlaceHorizontal(m.width, lipgloss.Center, helpBarStyle.Render(m.help.View(keymap))),
	)
}
