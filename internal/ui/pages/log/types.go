package log

import (
	"context"

	"github.com/amir20/dtop/internal/docker"
	"github.com/charmbracelet/bubbles/viewport"
)

type Model struct {
	ctx        context.Context
	client     *docker.Client
	width      int
	height     int
	container  *docker.Container
	cancel     context.CancelFunc
	viewport   viewport.Model
	logChannel <-chan docker.LogEntry
}
