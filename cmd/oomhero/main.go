package main

import (
	"log"
	"os"
	"strconv"
	"time"

	"oomhero/mem"
	"oomhero/proc"
)

var (
	warning  uint64 = 75
	critical uint64 = 90
)

func main() {
	parseEnv()
	log.Printf("warning set to %d%%", warning)
	log.Printf("critical set to %d%%", critical)

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
				// if there is no limit or we can't read it due
				// to permissions move on to the next process.
				if os.IsNotExist(err) || os.IsPermission(err) {
					continue
				}
				log.Printf("error reading mem: %s", err)
				continue
			}

			pct := (usage * 100) / limit
			log.Printf(
				"memory usage on pid %d's cgroup: %d%%",
				p.Pid, pct,
			)

			switch {
			case pct < warning:
				continue
			case pct < critical:
				warn = append(warn, p)
			default:
				crit = append(crit, p)
			}
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

func parseEnv() {
	warnAsString := os.Getenv("WARNING")
	if warnAsString != "" {
		tmp, err := strconv.ParseUint(warnAsString, 10, 64)
		if err != nil {
			log.Printf("error parsing %s as warning", warnAsString)
			log.Printf("using default(%d%%) instead", warning)
		} else {
			warning = tmp
		}
	}

	critAsString := os.Getenv("CRITICAL")
	if critAsString != "" {
		tmp, err := strconv.ParseUint(critAsString, 10, 64)
		if err != nil {
			log.Printf("error parsing %s as critical", critAsString)
			log.Printf("using default(%d%%) instead", critical)
		} else {
			critical = tmp
		}
	}
}
