package main

import (
	"fmt"
	"math/rand"
	"os"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/progress"
	"github.com/charmbracelet/bubbles/table"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

type containerRow struct {
	name     string
	cpuUsage float64 // 0.0 - 1.0
	mem      string
	status   string
	bar      tea.Model
}

type model struct {
	containers []containerRow
	table      table.Model
	width      int
}

type tickMsg struct{}

func main() {
	p := tea.NewProgram(initialModel(), tea.WithAltScreen())
	if _, err := p.Run(); err != nil {
		fmt.Println("Error:", err)
		os.Exit(1)
	}
}

func initialModel() model {
	rows := []containerRow{
		newContainer("nginx", 0.25, "24MB", "Running"),
		newContainer("redis", 0.50, "15MB", "Running"),
		newContainer("postgres", 0.75, "120MB", "Exited"),
		newContainer("app", 0.65, "35MB", "Running"),
	}

	dummyRows := []table.Row{}
	for _, c := range rows {
		dummyRows = append(dummyRows, table.Row{c.name, fmt.Sprintf("%.2f", c.cpuUsage), c.mem, c.status})
	}

	// Dummy table just for layout (we’ll override the View)
	tbl := table.New(
		table.WithColumns([]table.Column{
			{Title: "Container", Width: 20},
			{Title: "CPU", Width: 30},
			{Title: "Memory", Width: 12},
			{Title: "Status", Width: 12},
		}),
		table.WithRows(dummyRows),
		table.WithFocused(true),
		table.WithHeight(10),
	)

	tbl.SetStyles(table.DefaultStyles())

	return model{
		containers: rows,
		table:      tbl,
	}
}

func newContainer(name string, cpu float64, mem, status string) containerRow {
	bar := progress.New(progress.WithGradient("#00ff00", "#ff0000"))
	bar.SetPercent(cpu)
	return containerRow{name, cpu, mem, status, bar}
}

func (m model) Init() tea.Cmd {
	return tea.Tick(time.Second, func(t time.Time) tea.Msg {
		return tickMsg{}
	})
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {

	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.table.SetWidth(msg.Width - 6)

		// Resize columns proportionally
		total := m.width - 6
		cols := m.table.Columns()
		cols[0].Width = total / 5 // name
		cols[1].Width = total / 3 // cpu (bar)
		cols[2].Width = total / 6 // mem
		cols[3].Width = total - (cols[0].Width + cols[1].Width + cols[2].Width)
		m.table.SetColumns(cols)

	case tickMsg:
		cmds := []tea.Cmd{}

		for i := range m.containers {
			// Simulate CPU change
			delta := (rand.Float64() - 0.5) * 0.2
			m.containers[i].cpuUsage += delta
			if m.containers[i].cpuUsage < 0 {
				m.containers[i].cpuUsage = 0
			}
			if m.containers[i].cpuUsage > 1 {
				m.containers[i].cpuUsage = 1
			}

			// Convert to Progress model and set percent
			if bar, ok := m.containers[i].bar.(progress.Model); ok {
				cmd := bar.SetPercent(m.containers[i].cpuUsage)
				m.containers[i].bar = bar
				cmds = append(cmds, cmd)
			}
		}

		// Schedule next tick
		cmds = append(cmds, tea.Tick(time.Second, func(t time.Time) tea.Msg {
			return tickMsg{}
		}))

		return m, tea.Batch(cmds...)

	case tea.KeyMsg:
		if msg.String() == "q" {
			return m, tea.Quit
		}
	}

	cmds := []tea.Cmd{}

	var tblCmd tea.Cmd
	m.table, tblCmd = m.table.Update(msg)
	cmds = append(cmds, tblCmd)

	for i := range m.containers {
		var cmd tea.Cmd
		m.containers[i].bar, cmd = m.containers[i].bar.Update(msg)
		cmds = append(cmds, cmd)
	}

	return m, tea.Batch(cmds...)
}

func (m model) View() string {
	var sb strings.Builder

	headers := m.table.Columns()
	sb.WriteString(fmt.Sprintf(
		"%-*s %-*s %-*s %-*s\n",
		headers[0].Width, headers[0].Title,
		headers[1].Width, headers[1].Title,
		headers[2].Width, headers[2].Title,
		headers[3].Width, headers[3].Title,
	))

	cursor := m.table.Cursor()

	for i, c := range m.containers {
		bar := c.bar.View()
		line := fmt.Sprintf(
			"%-*s %-*s %-*s %-*s",
			headers[0].Width, c.name,
			headers[1].Width, bar,
			headers[2].Width, c.mem,
			headers[3].Width, c.status,
		)
		if i == cursor {
			line = selectedStyle.Render(line)
		}

		sb.WriteString(line)
		sb.WriteString("\n")
	}

	sb.WriteString(fmt.Sprintf("\nCursor: %d | Use ↑/↓ to select, q to quit.", cursor))

	return sb.String()
}

var selectedStyle = lipgloss.NewStyle().
	Background(lipgloss.Color("57")).
	Foreground(lipgloss.Color("230")).
	Bold(true)
