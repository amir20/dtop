package ui

import (
	"dtop/internal/docker"
	"dtop/internal/ui/components/table"
	"time"

	"github.com/charmbracelet/bubbles/help"
	"github.com/charmbracelet/bubbles/key"
	"github.com/charmbracelet/bubbles/progress"

	tea "github.com/charmbracelet/bubbletea"
)

type row struct {
	container     *docker.Container
	cpu           progress.Model
	mem           progress.Model
	bytesReceived uint64
	bytesSent     uint64
}

func newRow(container *docker.Container) row {
	return row{
		container: container,
		cpu:       progress.New(progress.WithDefaultGradient()),
		mem:       progress.New(progress.WithDefaultGradient()),
	}
}

type model struct {
	rows             map[string]row
	table            table.Model[row]
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
