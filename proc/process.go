package proc

import (
	"os"

	"github.com/ricardomaraschini/oomhero/mem"
)

type Process interface {
	Pid() int
	Signal(os.Signal) error
	MemoryUsagePercent() (uint64, error)
}

type OsProcess struct {
	process *os.Process
}

func NewOsProcess(p *os.Process) OsProcess {
	return OsProcess{
		process: p,
	}
}

func (p OsProcess) Pid() int {
	return p.process.Pid
}

func (p OsProcess) Signal(s os.Signal) error {
	return p.process.Signal(s)
}

func (p OsProcess) MemoryUsagePercent() (uint64, error) {
	limit, usage, err := mem.LimitAndUsageForProc(p.process)
	if err != nil {
		return 0, err
	}

	return (usage * 100) / limit, nil
}
