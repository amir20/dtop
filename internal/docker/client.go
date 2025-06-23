package docker

import (
	"context"

	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/api/types/events"
	"github.com/docker/docker/client"
)

type Client struct {
	cli *client.Client
}

func NewLocalClient() (*Client, error) {
	cli, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation(), client.WithUserAgent("Docker-Client/dtop"))
	if err != nil {
		return nil, err
	}

	return &Client{
		cli: cli,
	}, nil
}

func (d *Client) WatchContainers(ctx context.Context) (<-chan []*Container, error) {
	containerListOptions := container.ListOptions{
		All: true,
	}
	list, err := d.cli.ContainerList(ctx, containerListOptions)
	if err != nil {
		return nil, err
	}

	channel := make(chan []*Container)

	go func() {
		close(channel)
		var containers = make([]*Container, 0, len(list))
		for _, c := range list {
			container, err := d.InsepectContainer(ctx, c.ID)
			if err != nil {
				continue
			}
			containers = append(containers, &container)
		}

		select {
		case <-ctx.Done():
			return
		case channel <- containers:
		}

		dockerMessages, err := d.cli.Events(ctx, events.ListOptions{})

		for {
			select {
			case <-ctx.Done():
				return
			case err := <-err:
				panic(err)

			case message := <-dockerMessages:
				if message.Type == events.ContainerEventType && len(message.Actor.ID) > 0 {
					container, err := d.InsepectContainer(ctx, message.Actor.ID)
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

	return channel, nil
}

func (d *Client) InsepectContainer(ctx context.Context, id string) (Container, error) {
	json, err := d.cli.ContainerInspect(ctx, id)
	if err != nil {
		return Container{}, err
	}
	return newContainerFromJSON(json), nil
}

// func (d *DockerClient) ContainerStats(ctx context.Context, id string, stats chan<- container.ContainerStat) error {
// 	response, err := d.cli.ContainerStats(ctx, id, true)

// 	if err != nil {
// 		return err
// 	}

// 	defer response.Body.Close()
// 	decoder := json.NewDecoder(response.Body)
// 	var v *container.StatsResponse
// 	for {
// 		if err := decoder.Decode(&v); err != nil {
// 			return err
// 		}

// 		var (
// 			memPercent, cpuPercent float64
// 			mem, memLimit          float64
// 			previousCPU            uint64
// 			previousSystem         uint64
// 		)
// 		daemonOSType := response.OSType

// 		if daemonOSType != "windows" {
// 			previousCPU = v.PreCPUStats.CPUUsage.TotalUsage
// 			previousSystem = v.PreCPUStats.SystemUsage
// 			cpuPercent = calculateCPUPercentUnix(previousCPU, previousSystem, v)
// 			mem = calculateMemUsageUnixNoCache(v.MemoryStats)
// 			memLimit = float64(v.MemoryStats.Limit)
// 			memPercent = calculateMemPercentUnixNoCache(memLimit, mem)
// 		} else {
// 			cpuPercent = calculateCPUPercentWindows(v)
// 			mem = float64(v.MemoryStats.PrivateWorkingSet)
// 		}

// 		if cpuPercent > 0 || mem > 0 {
// 			select {
// 			case <-ctx.Done():
// 				return nil
// 			case stats <- container.ContainerStat{
// 				ID:            id,
// 				CPUPercent:    cpuPercent,
// 				MemoryPercent: memPercent,
// 				MemoryUsage:   mem,
// 			}:
// 			}
// 		}
// 	}
// }
