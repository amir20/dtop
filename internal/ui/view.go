package ui

import (
	"fmt"
	"strings"
)

func (m model) View() string {
	var sb strings.Builder
	lines := 0
	headers := m.table.Columns()
	sb.WriteString(fmt.Sprintf(
		"%-*s %-*s %-*s %-*s\n",
		headers[0].Width, headerStyle.Width(headers[0].Width).MaxWidth(headers[0].Width).MaxHeight(1).Render(headers[0].Title),
		headers[1].Width, headerStyle.Width(headers[1].Width).MaxWidth(headers[1].Width).MaxHeight(1).Render(headers[1].Title),
		headers[2].Width, headerStyle.Width(headers[2].Width).MaxWidth(headers[2].Width).MaxHeight(1).Render(headers[2].Title),
		headers[3].Width, headerStyle.Width(headers[3].Width).MaxWidth(headers[3].Width).MaxHeight(1).Render(headers[3].Title),
	))
	lines++

	cursor := m.table.Cursor()

	start := max(cursor-m.height/2, 0)
	end := min(start+m.height-2, len(m.orderedRows))

	for i := start; i < end; i++ {
		c := m.orderedRows[i]
		name := defaultStyle.Width(headers[0].Width).MaxWidth(headers[0].Width).MaxHeight(1).Render(c.container.Name)
		cpu := c.cpu.View()
		mem := c.mem.View()
		status := defaultStyle.Width(headers[3].Width).MaxWidth(headers[3].Width).MaxHeight(1).Render(c.container.State)

		if i == cursor {
			name = selectedStyle.Width(headers[0].Width).MaxWidth(headers[0].Width).MaxHeight(1).Render(c.container.Name)
			status = selectedStyle.Width(headers[3].Width).MaxWidth(headers[3].Width).MaxHeight(1).Render(c.container.State)
			bar := c.cpu
			bar.PercentageStyle = selectedStyle
			cpu = bar.View()
			bar = c.mem
			bar.PercentageStyle = selectedStyle
			mem = bar.View()
		}

		sb.WriteString(fmt.Sprintf(
			"%-*s %-*s %-*s %-*s\n",
			headers[0].Width, name,
			headers[1].Width, cpu,
			headers[2].Width, mem,
			headers[3].Width, status,
		))
		lines++
	}

	sb.WriteString(strings.Repeat("\n", max(m.height-lines-1, 0)))
	sb.WriteString(fmt.Sprintf("Use ↑/↓ to select, q to quit, a to toggle all containers"))

	return sb.String()
}
