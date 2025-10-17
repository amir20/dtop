package ui

import (
	"path"
	"slices"
	"sort"
	"strings"

	"github.com/amir20/dtop/config"
	"github.com/amir20/dtop/internal/ui/components/table"

	"github.com/charmbracelet/bubbles/key"
	tea "github.com/charmbracelet/bubbletea"

	"github.com/pkg/browser"
)

func (m model) updateColumnHeaders() model {

	columns := m.table.Columns()
	newColumns := make([]table.Column[row], 0, len(columns))

	for _, column := range columns {
		// Remove any existing arrows from column titles
		title := column.Title
		title = strings.TrimSuffix(title, " ↑")
		title = strings.TrimSuffix(title, " ↓")

		// Add arrow to the sorted column
		if strings.ToLower(title) == string(m.sortBy) {
			arrow := " ↑"
			if m.sortAsc {
				arrow = " ↓"
			}
			title = title + arrow
		}

		column.Title = title
		newColumns = append(newColumns, column)
	}

	m.table.SetColumns(newColumns)

	return m
}

func (m model) updateInternalRows() model {
	rows := make([]row, 0, len(m.rows))
	for _, r := range m.rows {
		if m.showAll || r.container.State == "running" {
			rows = append(rows, r)
		}
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
	case tickMsg:
		m.table.UpdateViewport()

		// Process all pending stats updates without triggering re-renders
		cmds := []tea.Cmd{tick()}
		processingStats := true
		for processingStats {
			select {
			case stat := <-m.stats:
				if row, exists := m.rows[stat.ID]; exists {
					row.stats.cpuPercent = stat.CPUPercent / 100
					row.stats.memPercent = stat.MemoryPercent / 100

					timeDelta := uint64(stat.Time.Sub(row.stats.lastUpdate).Seconds())
					if timeDelta > 0 && !row.stats.lastUpdate.IsZero() {
						currentBytesReceivedPerSecond := (stat.TotalNetworkReceived - row.stats.totalBytesReceived) / timeDelta
						currentBytesSentPerSecond := (stat.TotalNetworkTransmitted - row.stats.totalBytesSent) / timeDelta
						alpha := 0.75
						row.stats.bytesReceivedPerSecond = uint64(alpha*float64(currentBytesReceivedPerSecond) + (1-alpha)*float64(row.stats.bytesReceivedPerSecond))
						row.stats.bytesSentPerSecond = uint64(alpha*float64(currentBytesSentPerSecond) + (1-alpha)*float64(row.stats.bytesSentPerSecond))
					}
					row.stats.totalBytesReceived = stat.TotalNetworkReceived
					row.stats.totalBytesSent = stat.TotalNetworkTransmitted
					row.stats.lastUpdate = stat.Time
				}
			default:
				processingStats = false
			}
		}

		return m, tea.Batch(cmds...)

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

		// Invalidate caches since column widths changed
		for _, row := range m.rows {
			row.cache.id = ""
			row.cache.name = ""
			row.cache.status = ""
		}

		m = m.updateInternalRows()

		return m, nil

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
			m = m.updateColumnHeaders()
			return m, nil
		}
	}

	cmds := []tea.Cmd{}

	var cmd tea.Cmd
	m.table, cmd = m.table.Update(msg)
	cmds = append(cmds, cmd)

	if m.loading {
		m.spinner, cmd = m.spinner.Update(msg)
		cmds = append(cmds, cmd)
	}

	return m, tea.Batch(cmds...)
}
