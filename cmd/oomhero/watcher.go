package main

import (
	"log"
	"os"
	"time"

	"github.com/ricardomaraschini/oomhero/proc"
)

func CurrentTime() time.Time {
	return time.Now()
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
	process    proc.Process
	state      State
	lastSignal map[State]time.Time
}

func newProcessWatcher(p proc.Process) ProcessWatcher {
	return ProcessWatcher{
		process:    p,
		state:      Ok,
		lastSignal: make(map[State]time.Time),
	}
}

func (p *ProcessWatcher) isInState(s State) bool {
	return s == p.state
}

func (p *ProcessWatcher) transitionTo(s State) {
	log.Printf("process %d transitioning to state %v from %v", p.process.Pid(), s, p.state)

	p.state = Ok
}

func (p *ProcessWatcher) onCooldown(cooldown uint64) bool {
	if val, found := p.lastSignal[p.state]; found {
		elapsedSince := CurrentTime().Unix() - val.Unix()
		return elapsedSince < int64(cooldown)
	}
	return false
}

func (p *ProcessWatcher) signal() error {
	p.lastSignal[p.state] = CurrentTime()

	switch p.state {
	case Warning:
		return proc.SendWarningTo(p.process)
	case Critical:
		return proc.SendCriticalTo(p.process)
	default:
		return nil
	}

}

func watchProcesses(getProcesses func() ([]proc.Process, error)) {
	processSignalTracker := make(map[int]ProcessWatcher)

	for range time.NewTicker(time.Second).C {
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
				processSignalTracker[p.Pid()] = newProcessWatcher(p)
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
