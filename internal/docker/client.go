package docker

import (
	"context"
	"encoding/json"

	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/api/types/events"
	"github.com/docker/docker/api/types/filters"
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
		defer close(channel)
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

		dockerMessages, err := d.cli.Events(ctx, events.ListOptions{Filters: filters.NewArgs(
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

func (d *Client) WatchContainerStats(ctx context.Context) (<-chan ContainerStat, error) {
	stats := make(chan ContainerStat)

	list, err := d.cli.ContainerList(ctx, container.ListOptions{})
	if err != nil {
		return nil, err
	}

	go func() {
		defer close(stats)
		for _, c := range list {
			go d.streamStats(ctx, c.ID, stats)
		}

		dockerMessages, err := d.cli.Events(ctx, events.ListOptions{Filters: filters.NewArgs(
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
					if message.Action == "start" {
						go d.streamStats(ctx, message.Actor.ID, stats)
					}
				}
			}
		}
	}()

	return stats, nil
}

func (d *Client) streamStats(ctx context.Context, id string, stats chan<- ContainerStat) error {
	response, err := d.cli.ContainerStats(ctx, id, true)
	if err != nil {
		return err
	}
	defer response.Body.Close()

	decoder := json.NewDecoder(response.Body)
	var statsResponse *container.StatsResponse

	for {
		if err := decoder.Decode(&statsResponse); err != nil {
			return err
		}

		var cpuPercent, memPercent, mem float64
		if response.OSType != "windows" {
			cpuPercent = calculateCPUPercentUnix(
				statsResponse.PreCPUStats.CPUUsage.TotalUsage,
				statsResponse.PreCPUStats.SystemUsage,
				statsResponse,
			)
			mem = calculateMemUsageUnixNoCache(statsResponse.MemoryStats)
			memLimit := float64(statsResponse.MemoryStats.Limit)
			memPercent = calculateMemPercentUnixNoCache(memLimit, mem)
		} else {
			cpuPercent = calculateCPUPercentWindows(statsResponse)
			mem = float64(statsResponse.MemoryStats.PrivateWorkingSet)
		}

		if cpuPercent > 0 || mem > 0 {
			select {
			case <-ctx.Done():
				return nil
			case stats <- ContainerStat{
				ID:            statsResponse.ID[:12],
				CPUPercent:    cpuPercent,
				MemoryPercent: memPercent,
				MemoryUsage:   mem,
			}:
			}
		}
	}
}
