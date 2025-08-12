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

		tbl := m.table
		columns := tbl.Columns()
		newColumns := make([]table.Column[row], len(columns))

		for _, column := range columns {
			if strings.ToLower(column.Title) == string(m.sortBy) {
				if m.sortAsc {
					column.Title = lipgloss.JoinHorizontal(lipgloss.Left, column.Title, selectedStyle.Render(" ↓"))
				} else {
					column.Title = lipgloss.JoinHorizontal(lipgloss.Left, column.Title, selectedStyle.Render(" ↑"))
				}
			}
			newColumns = append(newColumns, column)
		}

		tbl.SetColumns(newColumns)

		return lipgloss.JoinVertical(
			lipgloss.Left, tbl.View(),
			lipgloss.PlaceHorizontal(m.width, lipgloss.Center, helpBarStyle.Render(m.help.View(keymap))),
		)
	}
}
