package main

import (
	"fmt"
	"log"
	"net/http"
	"os"

	"github.com/aitrack/server/internal/app"
	"github.com/aitrack/server/internal/config"
)

func main() {
	cfgPath := "config.yaml"
	if len(os.Args) > 1 {
		cfgPath = os.Args[1]
	}

	cfg, err := config.Load(cfgPath)
	if err != nil {
		log.Fatalf("load config: %v", err)
	}

	router, cleanup, err := app.Build(cfg)
	if err != nil {
		log.Fatalf("build: %v", err)
	}
	defer cleanup()

	addr := fmt.Sprintf(":%d", cfg.Server.Port)
	log.Printf("aitrack-server (Go) listening on %s", addr)
	if err := http.ListenAndServe(addr, router); err != nil {
		log.Fatalf("server: %v", err)
	}
}
