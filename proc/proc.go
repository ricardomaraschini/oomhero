package proc

import (
	"fmt"
	"io/ioutil"
	"os"
	"strconv"
	"syscall"
)

var (
	// WarningSignal is the signal sent to the process once we reach what
	// is considered a Warning threshold.
	WarningSignal = syscall.SIGUSR1

	// CriticalSignal is the signal sent to the process once we reach what
	// is considered a Critical threshold.
	CriticalSignal = syscall.SIGUSR2
)

// Others retun a list of all other ps. We do not return the current
// process information.
func Others() ([]*os.Process, error) {
	files, err := ioutil.ReadDir("/proc")
	if err != nil {
		return nil, err
	}

	ps := make([]*os.Process, 0)
	for _, file := range files {
		if !file.IsDir() {
			continue
		}

		pid, err := strconv.Atoi(file.Name())
		if err != nil {
			continue
		}

		if pid == os.Getpid() {
			continue
		}

		proccess, err := os.FindProcess(pid)
		if err != nil {
			return nil, err
		}

		ps = append(ps, proccess)
	}

	if len(ps) == 0 {
		return nil, fmt.Errorf("unable to find any process")
	}

	return ps, nil
}

// Warning sends a warning signal to a list or processes.
func Warning(ps []*os.Process) error {
	return sendSignal(WarningSignal, ps)
}

// Critical sends a critical signal to a list or processes.
func Critical(ps []*os.Process) error {
	return sendSignal(CriticalSignal, ps)
}

func sendSignal(sig syscall.Signal, ps []*os.Process) error {
	errs := &errors{}
	for _, p := range ps {
		if err := p.Signal(syscall.SIGUSR1); err != nil {
			errs.append(err)
		}
	}
	if errs.len() == 0 {
		return nil
	}
	return errs
}
