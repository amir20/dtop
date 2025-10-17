package ui

import (
	"context"

	"github.com/amir20/dtop/config"
	"github.com/amir20/dtop/internal/docker"
	"github.com/amir20/dtop/internal/ui/messages"
	"github.com/amir20/dtop/internal/ui/pages/list"
	logpage "github.com/amir20/dtop/internal/ui/pages/log"

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
}

func NewApp(ctx context.Context, client *docker.Client, defaultSort config.SortField) App {
	return App{
		ctx:         ctx,
		client:      client,
		currentPage: List,
		listPage:    list.NewModel(ctx, client, defaultSort),
		logPage:     logpage.NewModel(ctx, client),
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
	// Handle navigation message first
	if navMsg, ok := msg.(messages.NavigateToLogMsg); ok {
		a.currentPage = Log
		a.logPage = a.logPage.SetContainer(navMsg.ContainerID, navMsg.ContainerName)
		return a, nil
	}

	// Check for page navigation keys
	if keyMsg, ok := msg.(tea.KeyMsg); ok {
		switch keyMsg.String() {
		case "esc":
			// ESC to go back to list from any page
			if a.currentPage == Log {
				a.currentPage = List
				return a, nil
			}
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
