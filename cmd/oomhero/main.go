package main

import (
	"fmt"
	"log"
	"os"
	"time"

	"oomhero/mem"
	"oomhero/proc"
)

var (
	warning  uint64 = 80
	critical uint64 = 90
)

func main() {
	var limit uint64
	var usage uint64
	var pct uint64
	var err error
	var ps []*os.Process

	if limit, err = mem.Limit(); err != nil {
		log.Fatalf("error reading memory limit: %v", err)
	}

	for range time.NewTicker(time.Second).C {
		if usage, err = mem.Usage(); err != nil {
			log.Printf("error reading memory usage: %v", err)
			continue
		}

		pct = (usage * 100) / limit
		if pct < warning {
			log.Printf("current usage: %d%%", pct)
			continue
		}

		ps, err = proc.Others()
		if err != nil {
			log.Printf("error listing procs: %v", err)
			continue
		}

		if pct < critical {
			log.Printf("mem usage: %d%%, sending warning", pct)
			if err = proc.Warning(ps); err != nil {
				log.Printf("error signaling: %v", err)
			}
			continue
		}

		log.Printf("mem usage: %d%%, sending critical", pct)
		if err = proc.Critical(ps); err != nil {
			log.Printf("error signaling: %v", err)
		}
	}
}
