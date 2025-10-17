package ui

import "github.com/charmbracelet/lipgloss"

var redStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("1"))
var greenStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("2"))
var selectedStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("3"))
var helpBarStyle = lipgloss.NewStyle().Padding(0, 1)
var icon = lipgloss.NewStyle().Width(1).AlignHorizontal(lipgloss.Right).MaxWidth(1).Inline(true)
var idStyle = lipgloss.NewStyle().Width(13).MaxWidth(13).Inline(true)
