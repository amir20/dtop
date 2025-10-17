package styles

import "github.com/charmbracelet/lipgloss"

var RedStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("1"))
var GreenStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("2"))
var SelectedStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("3"))
var HelpBarStyle = lipgloss.NewStyle().Padding(0, 1)
var Icon = lipgloss.NewStyle().Width(1).AlignHorizontal(lipgloss.Right).MaxWidth(1).Inline(true)
var IdStyle = lipgloss.NewStyle().Width(13).MaxWidth(13).Inline(true)
