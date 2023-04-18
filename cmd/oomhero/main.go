package main

import (
	"log"
	"os"
	"strconv"
	"time"

	"github.com/ricardomaraschini/oomhero/proc"
)

var (
	warning  uint64 = 75
	critical uint64 = 90
	cooldown uint64 = 1
)

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

func main() {
	log.Printf("warning threshold set to %d%%", warning)
	log.Printf("critical threshold set to %d%%", critical)

	watchProcesses(time.NewTicker(time.Second).C, getOsProcesses)
}

func watchProcesses(ticks <-chan time.Time, getProcesses func() ([]proc.Process, error)) {
	processSignalTracker := make(map[int]*ProcessWatcher)

	for range ticks {
		ps, err := getProcesses()
		if err != nil {
			continue
		}

		for _, p := range ps {
			pct, err := p.MemoryUsagePercent()
			if err != nil {
				// if there is no limit or we can't read it due
				// to permissions move on to the next process.
				if os.IsNotExist(err) || os.IsPermission(err) {
					continue
				}
				log.Printf("error reading mem: %s", err)
				continue
			}

			log.Printf(
				"memory usage on pid %d's cgroup: %d%%",
				p.Pid(), pct,
			)

			if _, found := processSignalTracker[p.Pid()]; !found {
				watcher := newProcessWatcher(p)
				processSignalTracker[p.Pid()] = &watcher
			}
			processWatcher := processSignalTracker[p.Pid()]

			switch {
			case pct < warning:
				if !processWatcher.isInState(Ok) {
					processWatcher.transitionTo(Ok)
				}
			case pct >= warning && pct < critical:
				if !processWatcher.isInState(Warning) {
					processWatcher.transitionTo(Warning)
				}
				if !processWatcher.onCooldown(cooldown) {
					if err := processWatcher.signal(); err != nil {
						log.Printf("error signaling warning: %s", err)
					}
				}
			case pct >= critical:
				if !processWatcher.isInState(Critical) {
					processWatcher.transitionTo(Critical)
				}
				if !processWatcher.onCooldown(cooldown) {
					if err := processWatcher.signal(); err != nil {
						log.Printf("error signaling critical: %s", err)
					}
				}
			}
		}
	}
}

func getOsProcesses() ([]proc.Process, error) {
	ps, err := proc.Others()
	if err != nil {
		return nil, err
	}

	osps := make([]proc.Process, 0)
	for _, p := range ps {
		osps = append(osps, proc.NewOsProcess(p))
	}

	return osps, nil
}

type State int64

const (
	Ok State = iota
	Warning
	Critical
)

func (s State) String() string {
	switch s {
	case Ok:
		return "Ok"
	case Warning:
		return "Warning"
	case Critical:
		return "Critical"
	default:
		return "Unknown"
	}
}

type ProcessWatcher struct {
	process     proc.Process
	state       State
	lastSignals map[State]time.Time
}

func newProcessWatcher(p proc.Process) ProcessWatcher {
	return ProcessWatcher{
		process:     p,
		state:       Ok,
		lastSignals: make(map[State]time.Time),
	}
}

func (p *ProcessWatcher) isInState(s State) bool {
	return s == p.state
}

func (p *ProcessWatcher) transitionTo(s State) {
	log.Printf("process %d transitioning to state %v from %v", p.process.Pid(), s, p.state)

	p.state = s
}

func (p *ProcessWatcher) onCooldown(cooldown uint64) bool {
	if val, found := p.lastSignals[p.state]; found {
		elapsedSince := currentTime().Unix() - val.Unix()
		return elapsedSince < int64(cooldown)
	}
	return false
}

func (p *ProcessWatcher) signal() error {
	p.lastSignals[p.state] = currentTime()

	switch p.state {
	case Warning:
		return proc.SendWarningTo(p.process)
	case Critical:
		return proc.SendCriticalTo(p.process)
	default:
		return nil
	}

}

var currentTime = func() time.Time {
	return time.Now()
}
