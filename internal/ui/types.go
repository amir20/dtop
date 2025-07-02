package ui

import (
	"dtop/internal/docker"
	"dtop/internal/ui/components/table"
	"time"

	"github.com/charmbracelet/bubbles/help"
	"github.com/charmbracelet/bubbles/key"
	"github.com/charmbracelet/bubbles/progress"
	teaTable "github.com/charmbracelet/bubbles/table"
	"github.com/dustin/go-humanize"

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

func (r row) toTableRow() teaTable.Row {
	status := ""
	cpu := ""
	mem := ""
	style := redStyle
	icon := "⚫"
	if r.container.State == "running" {
		status = "Up " + humanize.RelTime(r.container.StartedAt, time.Now(), "", "")
		cpu = r.cpu.View()
		mem = r.mem.View()
		icon = "🟢"
		style = greenStyle
	} else if r.container.State == "exited" {
		status = "Exited " + humanize.RelTime(r.container.FinishedAt, time.Now(), "ago", "")
		icon = "🔴"
		style = redStyle
	}
	return teaTable.Row{icon, r.container.Name, r.container.ID, cpu, mem, style.Render(status)}
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
	keyMap           KeyMap
	help             help.Model
}

type tickMsg time.Time

func tick() tea.Cmd {
	return tea.Tick(time.Millisecond*100, func(t time.Time) tea.Msg {
		return tickMsg(t)
	})
}

type containers []*docker.Container

type KeyMap struct {
	LineUp   key.Binding
	LineDown key.Binding
	ShowAll  key.Binding
	Open     key.Binding
	Quit     key.Binding
}

func (km KeyMap) ShortHelp() []key.Binding {
	return []key.Binding{km.LineUp, km.LineDown, km.ShowAll, km.Open, km.Quit}
}

// FullHelp implements the KeyMap interface.
func (km KeyMap) FullHelp() [][]key.Binding {
	return [][]key.Binding{
		{km.LineUp, km.LineDown, km.ShowAll, km.Open, km.Quit},
		{},
	}
}

var defaultKeyMap = KeyMap{
	LineUp:   key.NewBinding(key.WithKeys("up", "k"), key.WithHelp("↑/k", "Move up")),
	LineDown: key.NewBinding(key.WithKeys("down", "j"), key.WithHelp("↓/j", "Move down")),
	ShowAll:  key.NewBinding(key.WithKeys("a"), key.WithHelp("a", "Toggle all")),
	Open:     key.NewBinding(key.WithKeys("o"), key.WithHelp("o", "Open")),
	Quit:     key.NewBinding(key.WithKeys("q"), key.WithHelp("q", "Quit")),
}
