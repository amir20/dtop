package config

type CliConfig struct {
	Hosts   []string `help:"Host configuration." default:"local" env:"DTOP_HOSTS" name:"host"`
	Version bool     `help:"Show version information." default:"false" name:"version" short:"v"`
}
