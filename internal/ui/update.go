package ui

import (
	"github.com/charmbracelet/bubbles/progress"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/pkg/browser"
)

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.table.SetWidth(msg.Width - 2)

		// Resize columns proportionally
		total := m.table.Width()
		cols := m.table.Columns()
		cols[0].Width = total / 4
		cols[1].Width = total / 4
		cols[2].Width = total / 4
		cols[3].Width = total / 4

		for i := range m.rows {
			m.rows[i].bar.Width = cols[1].Width
		}

		m.table.SetColumns(cols)
		return m, nil

	case tickMsg:
		return m, tick()

	case containers:
		for _, c := range msg {
			m.rows = append(m.rows, newRow(c))
		}

		return m, waitForContainerUpdate(m.containerWatcher)

	case tea.KeyMsg:
		switch msg.String() {
		case "q":
			return m, tea.Quit
		case "o":
			container := m.rows[m.table.Cursor()]
			browser.OpenURL("http://localhost:3100/container/" + container.container.ID)
			return m, nil
		}
	}

	cmds := []tea.Cmd{}

	var tblCmd tea.Cmd
	m.table, tblCmd = m.table.Update(msg)
	cmds = append(cmds, tblCmd)

	for i := range m.rows {
		var cmd tea.Cmd
		var barModel tea.Model
		barModel, cmd = m.rows[i].bar.Update(msg)
		m.rows[i].bar = barModel.(progress.Model)
		cmds = append(cmds, cmd)
	}

	return m, tea.Batch(cmds...)
}
