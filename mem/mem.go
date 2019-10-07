package mem

import (
	"fmt"
	"io/ioutil"
	"os"
	"strconv"
	"strings"
)

var (
	limitSuffixPath = "sys/fs/cgroup/memory/memory.limit_in_bytes"
	usageSuffixPath = "sys/fs/cgroup/memory/memory.usage_in_bytes"
)

// LimitAndUsageForProc returns memory limit and usage for cgroup where proc
// is running.
func LimitAndUsageForProc(proc *os.Process) (uint64, uint64, error) {
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
		limitSuffixPath,
	)
	return readUint64FromFile(limitFile)
}

// UsageForProc returns the amount of memory currently in use within the namespace
// where proc lives.
func UsageForProc(proc *os.Process) (uint64, error) {
	usageFile := fmt.Sprintf(
		"/proc/%d/root/%s",
		proc.Pid,
		usageSuffixPath,
	)
	return readUint64FromFile(usageFile)
}

func readUint64FromFile(fpath string) (uint64, error) {
	contentAsB, err := ioutil.ReadFile(fpath)
	if err != nil {
		return 0, err
	}
	contentAsStr := strings.TrimSuffix(string(contentAsB), "\n")
	return strconv.ParseUint(contentAsStr, 10, 64)
}
