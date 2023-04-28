package main

import (
	"log"
	"os"
	"strconv"
	"time"

	"github.com/ricardomaraschini/oomhero/proc"
)

var (
	warning  uint64        = 75
	critical uint64        = 90
	cooldown time.Duration = time.Second
)

func main() {
	watchProcesses(time.NewTicker(time.Second).C, getOsProcesses)
}

func watchProcesses(ticks <-chan time.Time, getProcesses func() ([]proc.Process, error)) {
	readThresholdsFromEnvironment()
	readCooldownFromEnvironment()

	log.Printf("warning threshold set to %d%%", warning)
	log.Printf("critical threshold set to %d%%", critical)
	log.Printf("cooldown set to %v", cooldown)

	processSignalTracker := make(map[int]*ProcessWatcher)

	for now := range ticks {
		ps, err := getProcesses()
		if err != nil {
			log.Printf("error listing procs: %v", err)
			continue
		}
		for _, p := range ps {
			pct, err := p.MemoryUsagePercent()
			if err != nil {
				log.Printf("error reading mem usage for pid %d: %s", p.Pid(), err)
				continue
			}
			log.Printf("memory usage on pid %d's cgroup: %d%%", p.Pid(), pct)
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
				if !processWatcher.onCooldown(now) {
					if err := processWatcher.signal(now); err != nil {
						log.Printf("error signaling warning: %s", err)
					}
				}
			case pct >= critical:
				if !processWatcher.isInState(Critical) {
					processWatcher.transitionTo(Critical)
				}
				if !processWatcher.onCooldown(now) {
					if err := processWatcher.signal(now); err != nil {
						log.Printf("error signaling critical: %s", err)
					}
				}
			}
		}
	}
}

// reads warning and critical from environment or use the default ones.
func readThresholdsFromEnvironment() {
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

func readCooldownFromEnvironment() {
	asString := os.Getenv("COOLDOWN")
	if asString == "" {
		return
	}

	val, err := time.ParseDuration(asString)
	if err != nil {
		log.Printf("error parsing COOLDOWN with time.ParseDuration: %v", err)
		log.Print("falling back to legacy behavior")
		legacyVal, err := strconv.ParseUint(asString, 10, 64)
		if err != nil {
			log.Printf("error parsing COOLDOWN as uint: %v", err)
			return
		}
		log.Print("detected usage of deprecated format of COOLDOWN, migrate to time.ParseDuration format as soon as possible")
		val = time.Duration(legacyVal) * time.Second
	}

	cooldownEnv := val

	if cooldownEnv < 0 {
		log.Print("cooldown must be a positive number")
		return
	}

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

func (p *ProcessWatcher) onCooldown(now time.Time) bool {
	if then, found := p.lastSignals[p.state]; found {
		elapsedSince := now.Sub(then)
		return elapsedSince < cooldown
	}
	return false
}

func (p *ProcessWatcher) signal(now time.Time) error {
	p.lastSignals[p.state] = now

	switch p.state {
	case Warning:
		return proc.SendWarningTo(p.process)
	case Critical:
		return proc.SendCriticalTo(p.process)
	default:
		return nil
	}

}
