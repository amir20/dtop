package config

type CliConfig struct {
	Hosts []string `help:"Host configuration." default:"local" env:"DTOP_HOSTS" name:"host"`
}
