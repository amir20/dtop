package ui

import (
	"context"
	"fmt"
	"os"
	"term-test/internal/docker"

	"github.com/charmbracelet/bubbles/table"
	tea "github.com/charmbracelet/bubbletea"
)

func NewModel(ctx context.Context, client *docker.Client) model {
	containerWatcher, err := client.WatchContainers(ctx)
	if err != nil {
		fmt.Println("Error:", err)
		os.Exit(1)
	}

	containers := <-containerWatcher

	var rows []row
	for _, c := range containers {
		rows = append(rows, newRow(c))
	}

	dummyRows := []table.Row{}
	for _, r := range rows {
		dummyRows = append(dummyRows, r.toTableRow())
	}

	tbl := table.New(
		table.WithColumns([]table.Column{
			{Title: "Container", Width: 20},
			{Title: "CPU", Width: 30},
			{Title: "Memory", Width: 12},
			{Title: "Status", Width: 12},
		}),
		table.WithRows(dummyRows),
		table.WithFocused(true),
		table.WithHeight(10),
	)

	tbl.SetStyles(table.DefaultStyles())

	return model{
		rows:             rows,
		table:            tbl,
		containerWatcher: containerWatcher,
	}
}

func waitForContainerUpdate(ch <-chan []*docker.Container) tea.Cmd {
	return func() tea.Msg {
		c := <-ch
		return containers(c)
	}
}

func (m model) Init() tea.Cmd {
	return tea.Batch(
		tick(),
		waitForContainerUpdate(m.containerWatcher),
	)
}
