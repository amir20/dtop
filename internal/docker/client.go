package docker

import (
	"context"
	"encoding/json"
	"time"

	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/api/types/events"
	"github.com/docker/docker/api/types/filters"
	"github.com/docker/docker/client"
)

type Client struct {
	clients []*client.Client
}

func NewMultiClient(clients ...*client.Client) *Client {
	for _, client := range clients {
		ctx, cancel := context.WithDeadline(context.Background(), time.Now().Add(10*time.Second))
		defer cancel()
		_, err := client.Ping(ctx)
		if err != nil {
			panic(err)
		}
	}
	return &Client{
		clients: clients,
	}
}

func (d *Client) WatchContainers(ctx context.Context) (<-chan []*Container, error) {
	containerListOptions := container.ListOptions{
		All: true,
	}
	channel := make(chan []*Container)

	for _, dockerClient := range d.clients {
		go func(client *client.Client) {
			list, err := client.ContainerList(ctx, containerListOptions)
			if err != nil {
				panic(err)
			}

			go func() {
				defer close(channel)
				var containers = make([]*Container, 0, len(list))
				for _, c := range list {
					container, err := inspectContainer(ctx, client, c.ID)
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

				dockerMessages, err := client.Events(ctx, events.ListOptions{Filters: filters.NewArgs(
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
							container, err := inspectContainer(ctx, client, message.Actor.ID)
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

func inspectContainer(ctx context.Context, client *client.Client, id string) (Container, error) {
	json, err := client.ContainerInspect(ctx, id)
	if err != nil {
		return Container{}, err
	}
	return newContainerFromJSON(json), nil
}

func (d *Client) WatchContainerStats(ctx context.Context) (<-chan ContainerStat, error) {
	stats := make(chan ContainerStat)
	for _, dockerClient := range d.clients {
		go func(client *client.Client) {
			list, err := client.ContainerList(ctx, container.ListOptions{})
			if err != nil {
				panic(err)
			}

			go func() {
				defer close(stats)
				for _, c := range list {
					go streamStats(ctx, client, c.ID, stats)
				}

				dockerMessages, err := client.Events(ctx, events.ListOptions{Filters: filters.NewArgs(
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
								go streamStats(ctx, client, message.Actor.ID, stats)
							}
						}
					}
				}
			}()
		}(dockerClient)
	}
	return stats, nil
}

func streamStats(ctx context.Context, client *client.Client, id string, stats chan<- ContainerStat) error {
	response, err := client.ContainerStats(ctx, id, true)
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
