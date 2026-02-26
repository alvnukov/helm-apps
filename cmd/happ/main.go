package main

import (
	"errors"
	"flag"
	"fmt"
	"os"

	"github.com/zol/helm-apps/cmd/happ/internal/app"
	"github.com/zol/helm-apps/cmd/happ/internal/cli"
)

func main() {
	cfg, err := cli.Parse(os.Args[1:], os.Stdout, os.Stderr)
	if err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return
		}
		fmt.Fprintf(os.Stderr, "happ failed: %v\n", err)
		os.Exit(1)
	}

	if err := (app.Service{}).Run(cfg); err != nil {
		fmt.Fprintf(os.Stderr, "happ failed: %v\n", err)
		os.Exit(1)
	}
}
