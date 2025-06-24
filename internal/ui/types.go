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
	mem       string
	bar       progress.Model
}

func newRow(container *docker.Container) row {
	bar := progress.New(progress.WithDefaultGradient())
	bar.SetPercent(0)
	return row{
		container: container,
		bar:       bar,
	}
}

func (r row) toTableRow() table.Row {
	return table.Row{r.container.Name, fmt.Sprintf("%.2f", r.bar.Percent()), r.mem, r.container.State}
}

type model struct {
	rows             map[string]*row
	orderedRows      []*row
	table            table.Model
	width            int
	height           int
	containerWatcher <-chan []*docker.Container
	stats            <-chan docker.ContainerStat
}

type tickMsg time.Time

func tick() tea.Cmd {
	return tea.Tick(time.Millisecond*100, func(t time.Time) tea.Msg {
		return tickMsg(t)
	})
}

type containers []*docker.Container
