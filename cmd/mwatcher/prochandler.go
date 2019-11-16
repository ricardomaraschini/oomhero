package main

import (
	"context"
	"log"
	"os"
	"sync"
	"time"
)

type procHandler struct {
	proc    *os.Process
	changed bool
	ctx     context.Context
	once    sync.Once
}

func (p *procHandler) Push(path string) {
	p.once.Do(func() {
		go p.loop()
	})
	log.Printf("changes on %s", path)
	p.changed = true
}

func (p *procHandler) loop() {
	ticker := time.NewTicker(3 * time.Second)
	defer ticker.Stop()
	for {
		select {
		case <-ticker.C:
			if !p.changed {
				continue
			}
			proc.SendRestart([]*os.Process{p.proc})
		case <-p.ctx.Done():
			return
		}
	}
}
