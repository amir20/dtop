package ui

import (
	"fmt"
	"strings"

	"github.com/amir20/dtop/internal/ui/components/table"
	"github.com/charmbracelet/lipgloss"
)

func (m model) View() string {
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

		// Only rebuild columns when sort state changes to avoid expensive UpdateViewport calls
		if m.sortBy != m.lastRenderedSortBy || m.sortAsc != m.lastRenderedSortAsc {
			tbl := m.table
			columns := tbl.Columns()
			newColumns := make([]table.Column[row], 0, len(columns))

			for _, column := range columns {
				if strings.ToLower(column.Title) == string(m.sortBy) {
					arrow := " ↑"
					if m.sortAsc {
						arrow = " ↓"
					}
					column.Title = lipgloss.JoinHorizontal(lipgloss.Left, column.Title, selectedStyle.Render(arrow))
				}
				newColumns = append(newColumns, column)
			}

			tbl.SetColumns(newColumns)
			m.table = tbl
			m.lastRenderedSortBy = m.sortBy
			m.lastRenderedSortAsc = m.sortAsc
		}

		return lipgloss.JoinVertical(
			lipgloss.Left, m.table.View(),
			lipgloss.PlaceHorizontal(m.width, lipgloss.Center, helpBarStyle.Render(m.help.View(keymap))),
		)
	}
}
