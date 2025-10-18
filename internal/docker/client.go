package docker

import (
	"context"
	"time"

	"github.com/amir20/dtop/config"
	"github.com/docker/docker/client"
)

type Client struct {
	hosts []Host
}

type Host struct {
	*client.Client
	config.HostConfig
	Local bool
}

func NewMultiClient(hosts ...Host) (*Client, error) {
	for _, client := range hosts {
		ctx, cancel := context.WithDeadline(context.Background(), time.Now().Add(10*time.Second))
		defer cancel()
		_, err := client.Ping(ctx)
		if err != nil {
			return nil, err
		}
	}
	return &Client{
		hosts: hosts,
	}, nil
}
