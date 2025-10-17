package ui

import (
	"time"

	"github.com/amir20/dtop/config"
	"github.com/amir20/dtop/internal/ui/components/table"

	"github.com/amir20/dtop/internal/docker"

	"github.com/charmbracelet/bubbles/help"
	"github.com/charmbracelet/bubbles/key"
	"github.com/charmbracelet/bubbles/spinner"

	tea "github.com/charmbracelet/bubbletea"
)

type rowStats struct {
	cpuPercent             float64
	memPercent             float64
	lastUpdate             time.Time
	totalBytesReceived     uint64
	totalBytesSent         uint64
	bytesReceivedPerSecond uint64
	bytesSentPerSecond     uint64
}

type rowCache struct {
	name   string
	id     string
	status string
}

type row struct {
	container *docker.Container
	stats     *rowStats
	cache     *rowCache
}

func newRow(container *docker.Container) row {
	return row{
		container: container,
		stats:     &rowStats{},
		cache:     &rowCache{},
	}
}

type model struct {
	rows             map[string]row
	table            table.Model[row]
	spinner          spinner.Model
	width            int
	height           int
	containerWatcher <-chan []*docker.Container
	stats            <-chan docker.ContainerStat
	keyMap           KeyMap
	help             help.Model
	sortBy           config.SortField
	loading          bool
	showAll          bool
	sortAsc          bool
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
	Sort     SortKeyMap
}

type SortKeyMap struct {
	Name   key.Binding
	Status key.Binding
}

func (km KeyMap) ShortHelp() []key.Binding {
	return []key.Binding{km.LineUp, km.LineDown, km.ShowAll, km.Open, km.Sort.Name, km.Sort.Status, km.Quit}
}

// FullHelp implements the KeyMap interface.
func (km KeyMap) FullHelp() [][]key.Binding {
	return [][]key.Binding{
		{km.LineUp, km.LineDown, km.ShowAll, km.Open, km.Sort.Name, km.Sort.Status, km.Quit},
		{},
	}
}

var defaultKeyMap = KeyMap{
	LineUp:   key.NewBinding(key.WithKeys("up", "k"), key.WithHelp("↑/k", "Move up")),
	LineDown: key.NewBinding(key.WithKeys("down", "j"), key.WithHelp("↓/j", "Move down")),
	ShowAll:  key.NewBinding(key.WithKeys("a"), key.WithHelp("a", "Toggle all")),
	Open:     key.NewBinding(key.WithKeys("o"), key.WithHelp("o", "Open Dozzle")),
	Quit:     key.NewBinding(key.WithKeys("q", "ctrl+c"), key.WithHelp("q", "Quit")),
	Sort: SortKeyMap{
		Name:   key.NewBinding(key.WithKeys("n"), key.WithHelp("n", "Sort by name")),
		Status: key.NewBinding(key.WithKeys("s"), key.WithHelp("s", "Sort by status")),
	},
}
