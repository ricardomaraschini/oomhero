package proc

import (
	"fmt"
	"io/ioutil"
	"log"
	"os"
	"strconv"
	"strings"
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

// CmdLine returns the command line for proc.
func CmdLine(proc *os.Process) (string, error) {
	cmdFile := fmt.Sprintf("/proc/%d/cmdline", proc.Pid)
	cmdAsB, err := ioutil.ReadFile(cmdFile)
	if err != nil {
		return "", err
	}
	cmdAsStr := strings.TrimSuffix(string(cmdAsB), "\n")
	return cmdAsStr, nil
}

// Others return a list of all other processes running on the system, excluding
// the current one.
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
	merrs := &MultiErrors{}
	for _, p := range ps {
		log.Printf("signaling %d with %q\n", p.Pid, sig.String())
		if err := p.Signal(sig); err != nil {
			merrs.es = append(merrs.es, err)
		}
	}
	if len(merrs.es) == 0 {
		return nil
	}
	return merrs
}
