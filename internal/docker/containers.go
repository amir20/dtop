package docker

import (
	"context"

	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/api/types/events"
	"github.com/docker/docker/api/types/filters"
)

func (d *Client) WatchContainers(ctx context.Context) (<-chan []*Container, error) {
	containerListOptions := container.ListOptions{
		All: true,
	}
	channel := make(chan []*Container)

	for _, dockerClient := range d.hosts {
		go func(host Host) {
			list, err := host.ContainerList(ctx, containerListOptions)
			if err != nil {
				panic(err)
			}

			go func() {
				defer close(channel)
				var containers = make([]*Container, 0, len(list))
				for _, c := range list {
					container, err := inspectContainer(ctx, host, c.ID)
					if err != nil {
						panic(err)
					}
					containers = append(containers, &container)
				}

				select {
				case <-ctx.Done():
					return
				case channel <- containers:
				}

				dockerMessages, err := host.Events(ctx, events.ListOptions{Filters: filters.NewArgs(
					filters.Arg("type", "container"),
					filters.Arg("event", "start"),
					filters.Arg("event", "stop"),
					filters.Arg("event", "die"),
				)})

				for {
					select {
					case <-ctx.Done():
						return
					case err := <-err:
						panic(err)

					case message := <-dockerMessages:
						if len(message.Actor.ID) > 0 {
							container, err := inspectContainer(ctx, host, message.Actor.ID)
							if err != nil {
								continue
							}

							select {
							case <-ctx.Done():
								return
							case channel <- []*Container{&container}:
							}
						}
					}
				}
			}()
		}(dockerClient)
	}

	return channel, nil
}

func inspectContainer(ctx context.Context, host Host, id string) (Container, error) {
	json, err := host.ContainerInspect(ctx, id)
	if err != nil {
		return Container{}, err
	}
	return newContainerFromJSON(json, host), nil
}
