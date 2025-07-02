package ui

func (m model) View() string {
	return m.table.View() + "\n" + helpBarStyle.Render(m.help.View(m.keyMap))
}
