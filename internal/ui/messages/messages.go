package messages

import (
	"github.com/amir20/dtop/internal/docker"
)

// NavigateToLogMsg is sent when user wants to view logs for a specific container
type ShowContainerMsg struct {
	Container *docker.Container
}
