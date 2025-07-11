package config

import (
	"github.com/alecthomas/kong"
)

type Cli struct {
	Hosts   []HostConfig `help:"List of hosts to connect to." name:"hosts" aliases:"host" default:"local" env:"DTOP_HOSTS"`
	Version bool         `help:"Show version information." default:"false" name:"version" short:"v"`
}

type HostConfig struct {
	Host   string `help:"Host address." name:"host"`
	Dozzle string `help:"Dozzle address." name:"dozzle"`
}

func (h *HostConfig) Decode(ctx *kong.DecodeContext) error {
	token, err := ctx.Scan.PopValue("string")
	if err != nil {
		return err
	}
	h.Host = token.String()
	return nil
}
