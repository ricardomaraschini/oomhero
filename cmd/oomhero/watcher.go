package main

import (
	"log"
	"os"

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
	process *os.Process
	state   State
	elapsed uint64
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
	p.elapsed = 0
}

func (p *ProcessWatcher) onCooldown(cooldown uint64) bool {
	return p.elapsed < cooldown
}

func (p *ProcessWatcher) tick() {
	p.elapsed++
}

func (p *ProcessWatcher) signal() error {
	switch p.state {
	case Warning:
		return proc.SendWarningTo(p.process)
	case Critical:
		return proc.SendCriticalTo(p.process)
	default:
		return nil
	}

}
