package main

import (
	"log"
	"os"
	"strconv"
	"time"

	"github.com/ricardomaraschini/oomhero/mem"
	"github.com/ricardomaraschini/oomhero/proc"
)

var (
	warning  uint64 = 75
	critical uint64 = 90
)

// reads warning and critical from environment or use the default ones.
func init() {
	warningEnv := envVarToUint64("WARNING", warning)
	criticalEnv := envVarToUint64("CRITICAL", critical)

	if warningEnv > 100 || criticalEnv > 100 {
		log.Print("warning and critical must be lower or equal to 100")
		return
	} else if warningEnv > criticalEnv {
		log.Print("warning must be lower or equal to critical")
		return
	}

	warning = warningEnv
	critical = criticalEnv
}

// envVarToUint64 converts the environment variable into a uint64, in case of
// error provided default value(def) is returned instead.
func envVarToUint64(name string, def uint64) uint64 {
	asString := os.Getenv(name)
	if asString == "" {
		return def
	}

	val, err := strconv.ParseUint(asString, 10, 64)
	if err != nil {
		return def
	}

	return val
}

func main() {
	log.Printf("warning threshold set to %d%%", warning)
	log.Printf("critical threshold set to %d%%", critical)

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
			if err := proc.SendWarning(warn); err != nil {
				log.Printf("error signaling warning: %s", err)
			}
		}

		if len(crit) > 0 {
			if err := proc.SendCritical(crit); err != nil {
				log.Printf("error signaling critical: %s", err)
			}
		}
	}
}
