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
func CmdLine(proc Process) (string, error) {
	cmdFile := fmt.Sprintf("/proc/%d/cmdline", proc.Pid())
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

func SendWarningTo(p Process) error {
	signal := resolveWarningSignal()
	return p.Signal(signal)
}

func resolveWarningSignal() syscall.Signal {
	envSignal := os.Getenv("WARNING_SIGNAL")
	if val, ok := supportedSignals[envSignal]; ok {
		return val
	}

	return syscall.SIGUSR1
}

func SendCriticalTo(p Process) error {
	signal := resolveCriticalSignal()
	return p.Signal(signal)
}

func resolveCriticalSignal() syscall.Signal {
	envSignal := os.Getenv("CRITICAL_SIGNAL")
	if val, ok := supportedSignals[envSignal]; ok {
		return val
	}

	return syscall.SIGUSR2
}
