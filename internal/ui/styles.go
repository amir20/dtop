package ui

import "github.com/charmbracelet/lipgloss"

var defaultStyle = lipgloss.NewStyle().Bold(false)
var headerStyle = lipgloss.NewStyle().Foreground(lipgloss.AdaptiveColor{Light: "#000", Dark: "#fff"}).Bold(true)
var selectedStyle = lipgloss.NewStyle().Foreground(lipgloss.AdaptiveColor{Light: "5", Dark: "5"}).Bold(true)
