package config

import (
	"net/http"

	"github.com/docker/cli/cli/connhelper"
	"github.com/docker/docker/client"
)

func NewLocalClient() (*client.Client, error) {
	cli, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation(), client.WithUserAgent("Docker-Client/dtop"))
	if err != nil {
		return nil, err
	}
	return cli, nil
}

func NewRemoteClient(host string) (*client.Client, error) {
	cli, err := client.NewClientWithOpts(client.WithHost(host), client.WithTLSClientConfigFromEnv(), client.WithUserAgent("Docker-Client/dtop"))
	if err != nil {
		return nil, err
	}
	return cli, nil
}

func NewSSHClient(host string) (*client.Client, error) {
	helper, err := connhelper.GetConnectionHelper(host)
	if err != nil {
		return nil, err
	}

	httpClient := &http.Client{
		Transport: &http.Transport{
			DialContext: helper.Dialer, // This sets up the tunnel over SSH
		},
	}

	cli, err := client.NewClientWithOpts(
		client.WithHTTPClient(httpClient),
		client.WithHost(helper.Host),
		client.WithDialContext(helper.Dialer),
		client.WithAPIVersionNegotiation(),
	)

	if err != nil {
		return nil, err
	}

	return cli, nil
}
