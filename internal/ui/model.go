package ui

import (
	"context"
	"dtop/internal/docker"
	"dtop/internal/ui/components/table"
	"fmt"
	"os"
	"path"
	"time"

	"github.com/charmbracelet/bubbles/help"
	teaTable "github.com/charmbracelet/bubbles/table"
	"github.com/charmbracelet/lipgloss"
	"github.com/dustin/go-humanize"
	"github.com/mattn/go-runewidth"

	tea "github.com/charmbracelet/bubbletea"
)

func NewModel(ctx context.Context, client *docker.Client) model {
	containerWatcher, err := client.WatchContainers(ctx)
	if err != nil {
		fmt.Println("Error:", err)
		os.Exit(1)
	}

	stats, err := client.WatchContainerStats(ctx)
	if err != nil {
		fmt.Println("Error:", err)
		os.Exit(1)
	}

	tbl := table.New(
		table.WithColumns([]table.Column[row]{
			{
				Title: "", Width: 2, Renderer: func(col table.Column[row], r row, selected bool) string {
					style := lipgloss.NewStyle().Width(col.Width).MaxWidth(col.Width).Inline(true)
					if r.container.State == "running" {
						return style.Render("🟢")
					}
					return style.Render("🔴")
				},
			},
			{
				Title: "NAME", Width: 10, Renderer: func(col table.Column[row], r row, selected bool) string {
					style := lipgloss.NewStyle().Width(col.Width).MaxWidth(col.Width).Inline(true)
					href := link(runewidth.Truncate(r.container.Name, col.Width, "…"), path.Join(r.container.Dozzle, "container", r.container.ID))
					return style.Render(href)
				},
			},
			{
				Title: "ID", Width: 13, Renderer: func(col table.Column[row], r row, selected bool) string {
					style := lipgloss.NewStyle().Width(col.Width).MaxWidth(col.Width).Inline(true)
					return style.Render(r.container.ID)
				},
			},
			{
				Title: "CPU", Width: 10, Renderer: func(col table.Column[row], r row, selected bool) string {
					if r.container.State == "running" {
						r.cpu.Width = col.Width
						return r.cpu.View()
					}
					return ""
				},
			},
			{
				Title: "MEMORY", Width: 10, Renderer: func(col table.Column[row], r row, selected bool) string {
					if r.container.State == "running" {
						r.mem.Width = col.Width
						return r.mem.View()
					}
					return ""
				},
			},
			{
				Title: "STATUS", Width: 10, Renderer: func(col table.Column[row], r row, selected bool) string {
					style := lipgloss.NewStyle().Width(col.Width).MaxWidth(col.Width).Inline(true)
					if r.container.State == "running" {
						return style.Render("Up " + humanize.RelTime(r.container.StartedAt, time.Now(), "", ""))
					}
					return style.Render("Exited " + humanize.RelTime(r.container.FinishedAt, time.Now(), "ago", ""))
				},
			},
		}),
		table.WithFocused[row](true),
		table.WithHeight[row](15),
	)

	tbl.SetStyles(teaTable.DefaultStyles())

	help := help.New()

	help.Styles.ShortKey = lipgloss.NewStyle().Bold(true)
	help.Styles.ShortDesc = lipgloss.NewStyle()

	if isSSHSession() {
		defaultKeyMap.Open.SetEnabled(false)
	}

	return model{
		rows:             make(map[string]row),
		table:            tbl,
		containerWatcher: containerWatcher,
		stats:            stats,
		keyMap:           defaultKeyMap,
		help:             help,
	}
}

func link(text, url string) string {
	return fmt.Sprintf("\033]8;;%s\033\\%s\033]8;;\033\\", url, text)
}

func waitForContainerUpdate(ch <-chan []*docker.Container) tea.Cmd {
	return func() tea.Msg {
		c := <-ch
		return containers(c)
	}
}

func waitForStatsUpdate(ch <-chan docker.ContainerStat) tea.Cmd {
	return func() tea.Msg {
		return <-ch
	}
}

func (m model) Init() tea.Cmd {
	return tea.Batch(
		tick(),
		waitForContainerUpdate(m.containerWatcher),
		waitForStatsUpdate(m.stats),
	)
}

func isSSHSession() bool {
	sshVars := []string{"SSH_CLIENT", "SSH_TTY", "SSH_CONNECTION"}

	for _, envVar := range sshVars {
		if os.Getenv(envVar) != "" {
			return true
		}
	}
	return false
}
