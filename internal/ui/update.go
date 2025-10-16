package ui

import (
	"path"
	"slices"
	"sort"

	"github.com/amir20/dtop/config"
	"github.com/amir20/dtop/internal/docker"

	"github.com/charmbracelet/bubbles/key"
	tea "github.com/charmbracelet/bubbletea"

	"github.com/pkg/browser"
	"github.com/samber/lo"
)

func (m model) updateInternalRows() model {
	rows := lo.Values(m.rows)

	if !m.showAll {
		rows = lo.Filter(rows, func(item row, index int) bool {
			return item.container.State == "running"
		})
	}

	var flipDesc = func(descSort bool) bool {
		if m.sortAsc {
			return !descSort
		}
		return descSort
	}

	sort.Slice(rows, func(i, j int) bool {
		switch m.sortBy {
		case config.SortByName:
			return flipDesc(rows[i].container.Name+rows[i].container.ID < rows[j].container.Name+rows[j].container.ID)
		case config.SortByStatus:
			return flipDesc(rows[i].container.CreatedAt.After(rows[j].container.CreatedAt))
		default:
			panic("unknown sort type")
		}
	})

	m.table.SetRows(rows)

	return m
}

var flexibleColumns = []string{"NAME", "CPU", "MEMORY", "STATUS", "NETWORK IO"}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height

		m.table.SetWidth(msg.Width)
		m.table.SetHeight(msg.Height - 1)

		total := m.table.Width()
		for _, col := range m.table.Columns() {
			if !slices.Contains(flexibleColumns, col.Title) {
				total -= col.Width
			}
		}

		cols := m.table.Columns()
		for i, col := range cols {
			if slices.Contains(flexibleColumns, col.Title) {
				cols[i].Width = total / len(flexibleColumns)
			}
		}

		return m, nil

	case tickMsg:
		m = m.updateInternalRows()
		return m, tick()

	case docker.ContainerStat:
		if row, exists := m.rows[msg.ID]; exists {
			// Store percentage values directly instead of using progress bar animation
			row.cpuPercent = msg.CPUPercent / 100
			row.memPercent = msg.MemoryPercent / 100

			timeDelta := uint64(msg.Time.Sub(row.lastUpdate).Seconds())
			if timeDelta > 0 && !row.lastUpdate.IsZero() {
				currentBytesReceivedPerSecond := (msg.TotalNetworkReceived - row.totalBytesReceived) / timeDelta
				currentBytesSentPerSecond := (msg.TotalNetworkTransmitted - row.totalBytesSent) / timeDelta
				alpha := 0.75
				row.bytesReceivedPerSecond = uint64(alpha*float64(currentBytesReceivedPerSecond) + (1-alpha)*float64(row.bytesReceivedPerSecond))
				row.bytesSentPerSecond = uint64(alpha*float64(currentBytesSentPerSecond) + (1-alpha)*float64(row.bytesSentPerSecond))
			}
			row.totalBytesReceived = msg.TotalNetworkReceived
			row.totalBytesSent = msg.TotalNetworkTransmitted
			row.lastUpdate = msg.Time
			m.rows[msg.ID] = row
			return m, waitForStatsUpdate(m.stats)
		}

		return m, waitForStatsUpdate(m.stats)

	case containers:
		for _, c := range msg {
			row := newRow(c)
			m.rows[c.ID] = row
		}
		m = m.updateInternalRows()
		m.loading = false
		return m, waitForContainerUpdate(m.containerWatcher)

	case tea.KeyMsg:
		switch {
		case key.Matches(msg, m.keyMap.LineUp):
			m.table.MoveUp(1)
			return m, nil
		case key.Matches(msg, m.keyMap.LineDown):
			m.table.MoveDown(1)
			return m, nil
		case key.Matches(msg, m.keyMap.Quit):
			return m, tea.Quit
		case key.Matches(msg, m.keyMap.Open):
			r := m.table.Rows()[m.table.Cursor()]
			browser.OpenURL(path.Join(r.container.Dozzle, "container", r.container.ID))
			return m, nil
		case key.Matches(msg, m.keyMap.ShowAll):
			m.showAll = !m.showAll
			m = m.updateInternalRows()
			return m, nil

		case key.Matches(msg, m.keyMap.Sort.Name, m.keyMap.Sort.Status):
			var field config.SortField
			switch {
			case key.Matches(msg, m.keyMap.Sort.Name):
				field = config.SortByName
			case key.Matches(msg, m.keyMap.Sort.Status):
				field = config.SortByStatus
			default:
				panic("unknown sort type")
			}

			if field == m.sortBy {
				m.sortAsc = !m.sortAsc
			} else {
				m.sortBy = field
			}
			m = m.updateInternalRows()
			return m, nil
		}
	}

	cmds := []tea.Cmd{}

	var cmd tea.Cmd
	m.table, cmd = m.table.Update(msg)
	cmds = append(cmds, cmd)

	// Don't update progress bars - they don't need animation and cause excessive re-renders
	// The bars are updated via SetPercent() in the ContainerStat case

	if m.loading {
		m.spinner, cmd = m.spinner.Update(msg)
		cmds = append(cmds, cmd)
	}

	return m, tea.Batch(cmds...)
}
