package list

import (
	"fmt"

	"github.com/amir20/dtop/internal/ui/styles"
	"github.com/charmbracelet/lipgloss"
)

func (m Model) View() string {
	keymap := m.keyMap
	rows := m.table.Rows()
	if m.loading {
		spinner := fmt.Sprintf("%s Loading", m.spinner.View())
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, spinner)
	} else {
		if keymap.Open.Enabled() {
			if m.table.Cursor() > -1 && m.table.Cursor() < len(rows) {
				selected := rows[m.table.Cursor()]
				if selected.container.Dozzle == "" {
					keymap.Open.SetEnabled(false)
				}
			}
		}

		return lipgloss.JoinVertical(
			lipgloss.Left, m.table.View(),
			lipgloss.PlaceHorizontal(m.width, lipgloss.Center, styles.HelpBarStyle.Render(m.help.View(keymap))),
		)
	}
}
