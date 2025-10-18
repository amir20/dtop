package log

import (
	"context"

	"github.com/amir20/dtop/internal/docker"
)

type Model struct {
	ctx       context.Context
	client    *docker.Client
	width     int
	height    int
	container *docker.Container
}
