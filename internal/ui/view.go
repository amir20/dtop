package ui

import (
	"fmt"
	"strings"
)

func (m model) View() string {
	var sb strings.Builder

	headers := m.table.Columns()
	sb.WriteString(fmt.Sprintf(
		"%-*s %-*s %-*s %-*s\n",
		headers[0].Width, headers[0].Title,
		headers[1].Width, headers[1].Title,
		headers[2].Width, headers[2].Title,
		headers[3].Width, headers[3].Title,
	))

	cursor := m.table.Cursor()

	for i, c := range m.rows {
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
	}

	sb.WriteString(fmt.Sprintf("\nCursor: %d | Use ↑/↓ to select, q to quit.", cursor))

	return sb.String()
}
