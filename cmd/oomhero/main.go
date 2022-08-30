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
	cooldown uint64 = 60
)

type ProcessWatch struct {
	process              *os.Process
	firedWarning         bool
	warningCooldownTime  uint64
	firedCritical        bool
	criticalCooldownTime uint64
}

// reads warning and critical from environment or use the default ones.
func init() {
	warningEnv := envVarToUint64("WARNING", warning)
	criticalEnv := envVarToUint64("CRITICAL", critical)
	cooldownEnv := envVarToUint64("COOLDOWN", cooldown)

	if warningEnv > 100 || criticalEnv > 100 {
		log.Print("warning and critical must be lower or equal to 100")
		return
	} else if warningEnv > criticalEnv {
		log.Print("warning must be lower or equal to critical")
		return
	}

	warning = warningEnv
	critical = criticalEnv
	cooldown = cooldownEnv
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

// Removes a process id from the array.
func filter(ss []*os.Process, remove *os.Process, test func(*os.Process, *os.Process) bool) (ret []*os.Process) {
	for _, s := range ss {
		if test(s, remove) {
			ret = append(ret, s)
		}
	}
	return
}

func main() {
	log.Printf("warning threshold set to %d%%", warning)
	log.Printf("critical threshold set to %d%%", critical)

	processSignalTracker := make(map[int]ProcessWatch)
	processFilter := func(checkP *os.Process, removeP *os.Process) bool { return checkP.Pid != removeP.Pid }

	for range time.NewTicker(time.Second).C {
		ps, err := proc.Others()
		if err != nil {
			log.Printf("Error listing procs: %v", err)
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

			if _, found := processSignalTracker[p.Pid]; !found {
				processSignalTracker[p.Pid] = ProcessWatch{
					process:              p,
					firedWarning:         false,
					firedCritical:        false,
					warningCooldownTime:  0,
					criticalCooldownTime: 0,
				}
			}
			processWatch := processSignalTracker[p.Pid]

			switch {
			case pct < warning:
				if processWatch.firedWarning {
					processWatch.firedWarning = false
					processWatch.warningCooldownTime = 0
					log.Printf("Process %p no longer in warning", &p.Pid)
					warn = filter(warn, p, processFilter)
				}
				if processWatch.firedCritical {
					processWatch.firedCritical = false
					processWatch.criticalCooldownTime = 0
					log.Printf("Process %p no longer in critical", &p.Pid)
					crit = filter(crit, p, processFilter)
				}
				continue
			case pct < critical: // We're at a warning stage, below critical but above warning
				processWatch.firedWarning = true
				if processWatch.warningCooldownTime == 0 {
					warn = append(warn, p)
				} else if processWatch.warningCooldownTime == cooldown {
					processWatch.warningCooldownTime = 0
				} else {
					// Wait one minute before firing again
					processWatch.warningCooldownTime++
				}
				// if we're transitioning down from Critical remove Critical
				if processWatch.firedCritical {
					processWatch.firedCritical = false
					processWatch.criticalCooldownTime = 0
					crit = filter(crit, p, processFilter)
				}
			default: // We're above the critical threshold
				processWatch.firedCritical = true
				if processWatch.criticalCooldownTime == 0 {
					crit = append(crit, p)
				} else if processWatch.criticalCooldownTime == cooldown {
					processWatch.criticalCooldownTime = 0
				} else {
					// Wait one minute before firing again
					processWatch.criticalCooldownTime++
				}
				// if we're transitioning from Warning remove Warning
				if processWatch.firedWarning {
					processWatch.firedWarning = false
					processWatch.warningCooldownTime = 0
					warn = filter(warn, p, processFilter)
				}
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
