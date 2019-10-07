package main

import (
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
	for range time.NewTicker(time.Second).C {
		ps, err := proc.Others()
		if err != nil {
			log.Printf("error listing procs: %v", err)
			continue
		}

		warn := make([]*os.Process, 0)
		crit := make([]*os.Process, 0)
		for _, p := range ps {
			limit, usage, err := mem.LimitAndUsageForProc(p)
			if err != nil {
				// if there is no limit or we can't read it we
				// just move on to the next process.
				if os.IsNotExist(err) || os.IsPermission(err) {
					continue
				}
				log.Printf("error reading mem: %s", err)
				continue
			}

			pct := (usage * 100) / limit
			log.Printf("mem usage on %d cgroup: %d%%", p.Pid, pct)
			if pct < warning {
				continue
			}

			if pct < critical {
				warn = append(warn, p)
				continue
			}

			crit = append(crit, p)
		}

		if len(warn) > 0 {
			if err := proc.Warning(warn); err != nil {
				log.Printf("error signaling warning: %s", err)
			}
		}

		if len(crit) > 0 {
			if err := proc.Critical(crit); err != nil {
				log.Printf("error signaling critical: %s", err)
			}
		}
	}
}
