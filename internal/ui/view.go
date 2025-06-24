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
		headers[0].Width, headers[0].Title,
		headers[1].Width, headers[1].Title,
		headers[2].Width, headers[2].Title,
		headers[3].Width, headers[3].Title,
	))
	lines++

	cursor := m.table.Cursor()

	start := max(cursor-3, 0)
	end := min(start+m.height-2, len(m.rows))

	for i := start; i < end; i++ {
		c := m.rows[i]
		name := c.container.Name
		cpu := c.bar.View()
		mem := c.mem
		status := c.container.State

		if i == cursor {
			name = selectedStyle.Width(headers[0].Width).Render(name)
			mem = selectedStyle.Width(headers[2].Width).Render(mem)
			status = selectedStyle.Width(headers[3].Width).Render(status)
			c.bar.PercentageStyle = selectedStyle
			cpu = c.bar.View()
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
	sb.WriteString(fmt.Sprintf("Cursor: %d | Use ↑/↓ to select, q to quit.", cursor))

	return sb.String()
}
