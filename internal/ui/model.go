package ui

import (
	"context"
	"fmt"
	"os"
	"path"
	"time"

	"github.com/amir20/dtop/config"
	"github.com/amir20/dtop/internal/docker"
	"github.com/amir20/dtop/internal/ui/components/table"

	"github.com/charmbracelet/bubbles/help"
	"github.com/charmbracelet/bubbles/progress"
	"github.com/charmbracelet/bubbles/spinner"
	teaTable "github.com/charmbracelet/bubbles/table"
	"github.com/charmbracelet/lipgloss"
	"github.com/dustin/go-humanize"
	"github.com/mattn/go-runewidth"

	tea "github.com/charmbracelet/bubbletea"
)

func NewModel(ctx context.Context, client *docker.Client, defaultSort config.SortField) model {
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

	progressBar := progress.New(progress.WithDefaultGradient())

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
					// Cache the base rendering (without selection styling)
					if r.cache.name == "" {
						style := lipgloss.NewStyle().Width(col.Width).MaxWidth(col.Width).Inline(true)
						value := r.container.Name
						if r.container.Dozzle != "" {
							value = link(runewidth.Truncate(value, col.Width, "…"), path.Join(r.container.Dozzle, "container", r.container.ID))
						} else {
							value = runewidth.Truncate(value, col.Width, "…")
						}
						r.cache.name = style.Render(value)
					}

					// Apply selection styling dynamically
					if selected {
						return selectedStyle.Render(r.cache.name)
					}
					return r.cache.name
				},
			},
			{
				Title: "ID", Width: 13, Renderer: func(col table.Column[row], r row, selected bool) string {
					// Cache the base rendering (without selection styling)
					if r.cache.id == "" {
						style := lipgloss.NewStyle().Width(col.Width).MaxWidth(col.Width).Inline(true)
						r.cache.id = style.Render(r.container.ID)
					}

					// Apply selection styling dynamically
					if selected {
						return selectedStyle.Render(r.cache.id)
					}
					return r.cache.id
				},
			},
			{
				Title: "CPU", Width: 10, Renderer: func(col table.Column[row], r row, selected bool) string {
					if r.container.State == "running" {
						bar := progressBar
						bar.Width = col.Width
						if selected {
							bar.PercentageStyle = selectedStyle
						}
						return bar.ViewAs(r.stats.cpuPercent)
					}
					return lipgloss.NewStyle().Width(col.Width).Inline(true).Render("")
				},
			},
			{
				Title: "MEMORY", Width: 10, Renderer: func(col table.Column[row], r row, selected bool) string {
					if r.container.State == "running" {
						bar := progressBar
						bar.Width = col.Width
						if selected {
							bar.PercentageStyle = selectedStyle
						}
						return bar.ViewAs(r.stats.memPercent)
					}
					return lipgloss.NewStyle().Width(col.Width).Inline(true).Render("")
				},
			},
			{
				Title: "NETWORK IO", Width: 10, Renderer: func(col table.Column[row], r row, selected bool) string {
					value := lipgloss.NewStyle().Width(col.Width).AlignHorizontal(lipgloss.Left).Inline(true).
						Render(
							fmt.Sprintf("↑ %-9s ↓ %s", humanize.Bytes(r.stats.bytesSentPerSecond)+"/s", humanize.Bytes(r.stats.bytesReceivedPerSecond)+"/s"),
						)
					if selected {
						value = selectedStyle.Render(value)
					}
					return value
				},
			},
			{
				Title: "STATUS", Width: 22, Renderer: func(col table.Column[row], r row, selected bool) string {
					// Cache the base rendering (without selection styling)
					if r.cache.status == "" {
						style := lipgloss.NewStyle().Width(col.Width).MaxWidth(col.Width).Inline(true)
						if r.container.State == "running" {
							r.cache.status = style.Render("Up " + humanize.RelTime(r.container.StartedAt, time.Now(), "", ""))
						} else {
							r.cache.status = style.Render("Exited " + humanize.RelTime(r.container.FinishedAt, time.Now(), "ago", ""))
						}
					}

					// Apply selection styling dynamically
					if selected {
						return selectedStyle.Render(r.cache.status)
					}
					return r.cache.status
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

	s := spinner.New()
	s.Spinner = spinner.Points
	s.Style = lipgloss.NewStyle().Foreground(lipgloss.Color("205"))

	m := model{
		rows:             make(map[string]row),
		table:            tbl,
		containerWatcher: containerWatcher,
		stats:            stats,
		keyMap:           defaultKeyMap,
		help:             help,
		spinner:          s,
		loading:          true,
		sortBy:           defaultSort,
		sortAsc:          false,
	}

	// Set initial column headers with sort arrow
	m = m.updateColumnHeaders()

	return m
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
		m.spinner.Tick,
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
