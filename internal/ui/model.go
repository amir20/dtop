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
				Title: "", Width: 1, Renderer: func(col table.Column[row], r row, selected bool) string {
					style := lipgloss.NewStyle().Width(col.Width).AlignHorizontal(lipgloss.Right).MaxWidth(col.Width).Inline(true)
					if r.container.State == "running" {
						return greenStyle.Render(style.Render("▶"))
					}
					return redStyle.Render(style.Render("⏹"))
				},
			},
			{
				Title: "NAME", Width: 10, Renderer: func(col table.Column[row], r row, selected bool) string {
					style := lipgloss.NewStyle().Width(col.Width).MaxWidth(col.Width).Inline(true)
					value := r.container.Name
					if r.container.Dozzle != "" {
						value = link(runewidth.Truncate(value, col.Width, "…"), path.Join(r.container.Dozzle, "container", r.container.ID))
					} else {
						value = runewidth.Truncate(value, col.Width, "…")
					}
					value = style.Render(value)
					if selected {
						value = selectedStyle.Render(value)
					}
					return value
				},
			},
			{
				Title: "ID", Width: 13, Renderer: func(col table.Column[row], r row, selected bool) string {
					style := lipgloss.NewStyle().Width(col.Width).MaxWidth(col.Width).Inline(true)
					value := style.Render(r.container.ID)
					if selected {
						value = selectedStyle.Render(value)
					}
					return value
				},
			},
			{
				Title: "CPU", Width: 10, Renderer: func(col table.Column[row], r row, selected bool) string {
					if r.container.State == "running" {
						bar := r.cpu
						bar.Width = col.Width
						if selected {
							bar.PercentageStyle = selectedStyle
						}
						return bar.View()
					}
					return lipgloss.NewStyle().Width(col.Width).Inline(true).Render("")
				},
			},
			{
				Title: "MEMORY", Width: 10, Renderer: func(col table.Column[row], r row, selected bool) string {
					if r.container.State == "running" {
						bar := r.mem
						bar.Width = col.Width
						if selected {
							bar.PercentageStyle = selectedStyle
						}
						return bar.View()
					}
					return lipgloss.NewStyle().Width(col.Width).Inline(true).Render("")
				},
			},
			{
				Title: "NETWORK IO", Width: 10, Renderer: func(col table.Column[row], r row, selected bool) string {
					value := lipgloss.NewStyle().Width(col.Width).AlignHorizontal(lipgloss.Left).Inline(true).
						Render(
							fmt.Sprintf("↑ %-6s  ↓ %-6s", humanize.Bytes(r.bytesSent), humanize.Bytes(r.bytesReceived)),
						)
					if selected {
						value = selectedStyle.Render(value)
					}
					return value
				},
			},
			{
				Title: "STATUS", Width: 22, Renderer: func(col table.Column[row], r row, selected bool) string {
					style := lipgloss.NewStyle().Width(col.Width).MaxWidth(col.Width).Inline(true)
					var value string
					if r.container.State == "running" {
						value = style.Render("Up " + humanize.RelTime(r.container.StartedAt, time.Now(), "", ""))
					} else {
						value = style.Render("Exited " + humanize.RelTime(r.container.FinishedAt, time.Now(), "ago", ""))
					}
					if selected {
						value = selectedStyle.Render(value)
					}
					return value
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
