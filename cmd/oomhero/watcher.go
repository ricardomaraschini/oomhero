package main

import (
	"log"
	"os"
	"time"

	"github.com/ricardomaraschini/oomhero/proc"
)

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
	process    *os.Process
	state      State
	lastSignal map[State]time.Time
}

func newProcessWatcher(p *os.Process) ProcessWatcher {
	return ProcessWatcher{
		process: p,
		state:   Ok,
	}
}

func (p *ProcessWatcher) isInState(s State) bool {
	return s == p.state
}

func (p *ProcessWatcher) transitionTo(s State) {
	log.Printf("process %d transitioning to state %v from %v", p.process.Pid, s, p.state)

	p.state = Ok
}

func (p *ProcessWatcher) onCooldown(cooldown uint64) bool {
	if val, found := p.lastSignal[p.state]; found {
		elapsedSince := time.Now().Unix() - val.Unix()
		return elapsedSince < int64(cooldown)
	}
	return false
}

func (p *ProcessWatcher) signal() error {
	p.lastSignal[p.state] = time.Now()

	switch p.state {
	case Warning:
		return proc.SendWarningTo(p.process)
	case Critical:
		return proc.SendCriticalTo(p.process)
	default:
		return nil
	}

}

func watchProcesses() {

}
