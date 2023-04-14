package proc

import (
	"fmt"
	"io/ioutil"
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

	supportedSignals = map[string]syscall.Signal{
		"SIGABRT": syscall.SIGABRT,
		"SIGCONT": syscall.SIGCONT,
		"SIGHUP":  syscall.SIGHUP,
		"SIGINT":  syscall.SIGINT,
		"SIGIOT":  syscall.SIGIOT,
		"SIGKILL": syscall.SIGKILL,
		"SIGQUIT": syscall.SIGQUIT,
		"SIGSTOP": syscall.SIGSTOP,
		"SIGTERM": syscall.SIGTERM,
		"SIGTSTP": syscall.SIGTSTP,
		"SIGUSR1": syscall.SIGUSR1,
		"SIGUSR2": syscall.SIGUSR2,
	}
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

	if len(ps) == 0 {
		return nil, fmt.Errorf("unable to find any process")
	}

	return ps, nil
}

// SendWarning sends a warning signal to a list or processes.
func SendWarning(ps []*os.Process) error {
	signal := resolveWarningSignal()
	return sendSignal(signal, ps)
}

func resolveWarningSignal() syscall.Signal {
	envSignal := os.Getenv("WARNING_SIGNAL")
	if val, ok := supportedSignals[envSignal]; ok {
		return val
	}

	return syscall.SIGUSR1
}

// SendCritical sends a critical signal to a list or processes.
func SendCritical(ps []*os.Process) error {
	signal := resolveCriticalSignal()
	return sendSignal(signal, ps)
}

func resolveCriticalSignal() syscall.Signal {
	envSignal := os.Getenv("CRITICAL_SIGNAL")
	if val, ok := supportedSignals[envSignal]; ok {
		return val
	}

	return syscall.SIGUSR2
}

func sendSignal(sig syscall.Signal, ps []*os.Process) error {
	merrs := &MultiErrors{}
	for _, p := range ps {
		if err := p.Signal(sig); err != nil {
			merrs.es = append(merrs.es, err)
		}
	}
	if len(merrs.es) == 0 {
		return nil
	}
	return merrs
}
