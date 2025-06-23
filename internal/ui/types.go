package ui

import (
	"fmt"
	"term-test/internal/docker"
	"time"

	"github.com/charmbracelet/bubbles/progress"
	"github.com/charmbracelet/bubbles/table"
	tea "github.com/charmbracelet/bubbletea"
)

type row struct {
	container *docker.Container
	cpuUsage  float64
	mem       string
	bar       progress.Model
}

func newRow(container *docker.Container) row {
	bar := progress.New(progress.WithDefaultGradient())
	bar.SetPercent(0)
	return row{
		container: container,
		cpuUsage:  0,
		bar:       bar,
	}
}

func (r row) toTableRow() table.Row {
	return table.Row{r.container.Name, fmt.Sprintf("%.2f", r.cpuUsage), r.mem, r.container.State}
}

type model struct {
	rows             []row
	table            table.Model
	width            int
	containerWatcher <-chan []*docker.Container
}

type tickMsg time.Time

func tick() tea.Cmd {
	return tea.Tick(time.Millisecond*100, func(t time.Time) tea.Msg {
		return tickMsg(t)
	})
}

type containers []*docker.Container
