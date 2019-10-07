package mem

import (
	"fmt"
	"io/ioutil"
	"os"
	"strconv"
	"strings"
)

var (
	limitSuffix = "sys/fs/cgroup/memory/memory.limit_in_bytes"
	usageSuffix = "sys/fs/cgroup/memory/memory.usage_in_bytes"
)

// LimitAndUsage returns memory limit and usage for cgroup where proc runs.
func LimitAndUsage(proc *os.Process) (uint64, uint64, error) {
	limit, err := LimitForProc(proc)
	if err != nil {
		return 0, 0, err
	}

	usage, err := UsageForProc(proc)
	if err != nil {
		return 0, 0, err
	}

	return limit, usage, nil
}

// LimitForProc returns the max memory on proc cgroup.
func LimitForProc(proc *os.Process) (uint64, error) {
	limitFile := fmt.Sprintf(
		"/proc/%d/root/%s",
		proc.Pid,
		limitSuffix,
	)
	limitAsB, err := ioutil.ReadFile(limitFile)
	if err != nil {
		return 0, err
	}
	limitAsStr := strings.TrimSuffix(string(limitAsB), "\n")
	return strconv.ParseUint(limitAsStr, 10, 64)
}

// UsageForProc returns the amount of memory currently in use within the namespace
// where proc lives.
func UsageForProc(proc *os.Process) (uint64, error) {
	usageFile := fmt.Sprintf(
		"/proc/%d/root/%s",
		proc.Pid,
		usageSuffix,
	)
	usageAsB, err := ioutil.ReadFile(usageFile)
	if err != nil {
		return 0, err
	}
	usageAsStr := strings.TrimSuffix(string(usageAsB), "\n")
	return strconv.ParseUint(usageAsStr, 10, 64)
}
