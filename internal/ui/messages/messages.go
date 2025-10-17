package messages

// NavigateToLogMsg is sent when user wants to view logs for a specific container
type NavigateToLogMsg struct {
	ContainerID   string
	ContainerName string
}
