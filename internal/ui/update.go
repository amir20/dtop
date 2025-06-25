package ui

import (
	"dtop/internal/docker"
	"sort"

	"github.com/charmbracelet/bubbles/progress"
	"github.com/charmbracelet/bubbles/table"
	tea "github.com/charmbracelet/bubbletea"

	"github.com/pkg/browser"
	"github.com/samber/lo"
)

func (m model) updateInternalRows() model {
	values := lo.Values(m.rows)

	if !m.showAll {
		values = lo.Filter(values, func(item *row, index int) bool {
			return item.container.State == "running"
		})
	}

	sort.Slice(values, func(i, j int) bool {
		return values[i].container.Created.After(values[j].container.Created)
	})

	dummyRows := []table.Row{}
	for _, r := range values {
		dummyRows = append(dummyRows, r.toTableRow())
	}
	m.table.SetRows(dummyRows)

	m.orderedRows = values

	return m
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		m.table.SetWidth(msg.Width)

		total := m.table.Width()
		cols := m.table.Columns()
		cols[0].Width = total / 4
		cols[1].Width = total / 4
		cols[2].Width = total / 4
		cols[3].Width = total / 4

		for _, row := range m.rows {
			row.cpu.Width = cols[1].Width
			row.mem.Width = cols[2].Width
		}

		m.table.SetColumns(cols)
		return m, nil

	case tickMsg:
		return m, tick()

	case docker.ContainerStat:
		if row, exists := m.rows[msg.ID]; exists {
			return m, tea.Batch(row.cpu.SetPercent(msg.CPUPercent/100), row.mem.SetPercent(msg.MemoryPercent/100), waitForStatsUpdate(m.stats))
		}

		return m, waitForStatsUpdate(m.stats)

	case containers:
		cols := m.table.Columns()
		for _, c := range msg {
			row := newRow(c)
			row.cpu.Width = cols[1].Width
			row.mem.Width = cols[2].Width
			m.rows[c.ID] = &row
		}
		m = m.updateInternalRows()
		return m, waitForContainerUpdate(m.containerWatcher)

	case tea.KeyMsg:
		switch msg.String() {
		case "q":
			return m, tea.Quit
		case "o":
			container := m.orderedRows[m.table.Cursor()]
			browser.OpenURL("http://localhost:3100/container/" + container.container.ID)
			return m, nil
		case "a":
			m.showAll = !m.showAll
			m = m.updateInternalRows()
			return m, nil
		}
	}

	cmds := []tea.Cmd{}

	var tblCmd tea.Cmd
	m.table, tblCmd = m.table.Update(msg)
	cmds = append(cmds, tblCmd)

	for _, row := range m.rows {
		var cmd tea.Cmd
		var cpu tea.Model
		cpu, cmd = row.cpu.Update(msg)
		row.cpu = cpu.(progress.Model)
		var mem tea.Model
		mem, cmd = row.mem.Update(msg)
		row.mem = mem.(progress.Model)
		cmds = append(cmds, cmd)
	}

	return m, tea.Batch(cmds...)
}
