package ui

import (
	"context"

	"github.com/amir20/dtop/config"
	"github.com/amir20/dtop/internal/docker"

	"github.com/amir20/dtop/internal/ui/pages/list"
	"github.com/amir20/dtop/internal/ui/pages/log"

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
	logPage     log.Model
}

func NewApp(ctx context.Context, client *docker.Client, defaultSort config.SortField) App {
	return App{
		ctx:         ctx,
		client:      client,
		currentPage: List,
		listPage:    list.NewModel(ctx, client, defaultSort),
		logPage:     log.NewModel(ctx, client),
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
	// Check for page navigation keys first
	if keyMsg, ok := msg.(tea.KeyMsg); ok {
		switch keyMsg.String() {
		case "L":
			// Switch to log page
			a.currentPage = Log
			return a, nil
		case "l":
			// Switch to list page
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
		a.logPage = model.(log.Model)
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
