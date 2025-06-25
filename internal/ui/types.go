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

	cpu progress.Model
	mem progress.Model
}

func newRow(container *docker.Container) row {
	cpu := progress.New(progress.WithDefaultGradient())
	cpu.SetPercent(0)
	mem := progress.New(progress.WithDefaultGradient())
	mem.SetPercent(0)
	return row{
		container: container,
		cpu:       cpu,
		mem:       mem,
	}
}

func (r row) toTableRow() table.Row {
	return table.Row{r.container.Name, fmt.Sprintf("%.2f", r.cpu.Percent()), fmt.Sprintf("%.2f", r.mem.Percent()), r.container.State}
}

type model struct {
	rows             map[string]*row
	orderedRows      []*row
	table            table.Model
	width            int
	height           int
	containerWatcher <-chan []*docker.Container
	stats            <-chan docker.ContainerStat
	showAll          bool
}

type tickMsg time.Time

func tick() tea.Cmd {
	return tea.Tick(time.Millisecond*100, func(t time.Time) tea.Msg {
		return tickMsg(t)
	})
}

type containers []*docker.Container
