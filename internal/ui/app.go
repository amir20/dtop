package ui

import (
	"context"

	"github.com/amir20/dtop/config"
	"github.com/amir20/dtop/internal/docker"
	"github.com/amir20/dtop/internal/ui/messages"
	"github.com/amir20/dtop/internal/ui/pages/list"
	logpage "github.com/amir20/dtop/internal/ui/pages/log"

	"github.com/charmbracelet/bubbles/key"
	tea "github.com/charmbracelet/bubbletea"
)

type PageType int

const (
	List PageType = iota
	Log
)

type App struct {
	ctx         context.Context
	client      *docker.Client
	currentPage PageType
	listPage    list.Model
	logPage     logpage.Model
	quitKey     key.Binding
	backKey     key.Binding
}

var (
	defaultQuitKey = key.NewBinding(
		key.WithKeys("q", "ctrl+c"),
		key.WithHelp("q", "Quit"),
	)
	defaultBackKey = key.NewBinding(
		key.WithKeys("esc", "left"),
		key.WithHelp("esc/left", "Go back"),
	)
)

func NewApp(ctx context.Context, client *docker.Client, defaultSort config.SortField) App {
	return App{
		ctx:         ctx,
		client:      client,
		currentPage: List,
		listPage:    list.NewModel(ctx, client, defaultSort),
		logPage:     logpage.NewModel(ctx, client),
		quitKey:     defaultQuitKey,
		backKey:     defaultBackKey,
	}
}

func (a App) Init() tea.Cmd {
	// Initialize the current page
	switch a.currentPage {
	case List:
		return a.listPage.Init()
	case Log:
		return a.logPage.Init()
	}
	return nil
}

func (a App) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case messages.ShowContainerMsg:
		a.currentPage = Log
		a.logPage = a.logPage.SetContainer(msg.Container)
		return a, nil

	case tea.KeyMsg:
		switch {
		case key.Matches(msg, a.quitKey):
			return a, tea.Quit
		case key.Matches(msg, a.backKey) && a.currentPage == Log:
			// Handle ESC to go back to list from any page
			a.currentPage = List
			return a, nil
		}
	}

	// Delegate to current page
	var cmd tea.Cmd
	switch a.currentPage {
	case List:
		var model tea.Model
		model, cmd = a.listPage.Update(msg)
		a.listPage = model.(list.Model)
	case Log:
		var model tea.Model
		model, cmd = a.logPage.Update(msg)
		a.logPage = model.(logpage.Model)
	}

	return a, cmd
}

func (a App) View() string {
	switch a.currentPage {
	case List:
		return a.listPage.View()
	case Log:
		return a.logPage.View()
	}
	return ""
}
