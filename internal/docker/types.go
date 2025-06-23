package docker

import (
	"strings"
	"time"

	docker "github.com/docker/docker/api/types/container"
)

type Container struct {
	ID          string            `json:"id"`
	Name        string            `json:"name"`
	Image       string            `json:"image"`
	Command     string            `json:"command"`
	Created     time.Time         `json:"created"`
	StartedAt   time.Time         `json:"startedAt"`
	FinishedAt  time.Time         `json:"finishedAt"`
	State       string            `json:"state"`
	Health      string            `json:"health,omitempty"`
	MemoryLimit uint64            `json:"memoryLimit"`
	CPULimit    float64           `json:"cpuLimit"`
	Labels      map[string]string `json:"labels,omitempty"`
}

func newContainerFromJSON(c docker.InspectResponse) Container {
	name := "no name"
	if c.Config.Labels["dev.dozzle.name"] != "" {
		name = c.Config.Labels["dev.dozzle.name"]
	} else if len(c.Name) > 0 {
		name = strings.TrimPrefix(c.Name, "/")
	}

	container := Container{
		ID:          c.ID[:12],
		Name:        name,
		Image:       c.Config.Image,
		Command:     strings.Join(c.Config.Entrypoint, " ") + " " + strings.Join(c.Config.Cmd, " "),
		State:       c.State.Status,
		Labels:      c.Config.Labels,
		MemoryLimit: uint64(c.HostConfig.Memory),
		CPULimit:    float64(c.HostConfig.NanoCPUs) / 1e9,
	}

	if createdAt, err := time.Parse(time.RFC3339Nano, c.Created); err == nil {
		container.Created = createdAt.UTC()
	}

	if startedAt, err := time.Parse(time.RFC3339Nano, c.State.StartedAt); err == nil {
		container.StartedAt = startedAt.UTC()
	}

	if stoppedAt, err := time.Parse(time.RFC3339Nano, c.State.FinishedAt); err == nil {
		container.FinishedAt = stoppedAt.UTC()
	}

	if c.State.Health != nil {
		container.Health = strings.ToLower(c.State.Health.Status)
	}

	return container
}

type ContainerEvent struct {
	Name            string            `json:"name"`
	Host            string            `json:"host"`
	ActorID         string            `json:"actorId"`
	ActorAttributes map[string]string `json:"actorAttributes,omitempty"`
	Time            time.Time         `json:"time"`
}
