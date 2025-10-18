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
	"github.com/charmbracelet/lipgloss"
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
	width       int
	height      int
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
		quitKey:     defaultQuitKey,
		backKey:     defaultBackKey,
	}
}

func (a App) activePage() tea.Model {
	switch a.currentPage {
	case List:
		return a.listPage
	case Log:
		return a.logPage
	}
	return nil
}

func (a App) Init() tea.Cmd {
	return a.activePage().Init()
}

func (a App) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case messages.ShowContainerMsg:
		a.currentPage = Log
		a.logPage = logpage.NewModel(a.ctx, a.client, msg.Container, a.width, a.height-1)
		return a, tea.Batch(a.logPage.Init())

	case tea.WindowSizeMsg:
		a.width = msg.Width
		a.height = msg.Height
		// Check if the current page implements StatusBar interface
		// If it does, subtract 1 from height to account for the status bar
		if _, ok := a.activePage().(StatusBar); ok {
			msg.Height--
		}

		a.activePage().Update(msg)

	case tea.KeyMsg:
		switch {
		case key.Matches(msg, a.quitKey):
			return a, tea.Quit
		case key.Matches(msg, a.backKey) && a.currentPage == Log:
			// Call Destroy if the current page implements it
			if destroyable, ok := a.activePage().(Destroy); ok {
				destroyable.Destroy()
			}

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
	content := a.activePage().View()
	if statusBarPage, ok := a.activePage().(StatusBar); ok {
		return lipgloss.JoinVertical(lipgloss.Left, content, statusBarPage.StatusBar())
	}

	return content
}
