package mem

import (
	"bytes"
	"fmt"
	"io/ioutil"
	"os"
	"strconv"
	"strings"
)

var (
	limitSuffixPathCgroupV1 = "sys/fs/cgroup/memory/memory.limit_in_bytes"
	usageSuffixPathCgroupV1 = "sys/fs/cgroup/memory/memory.usage_in_bytes"
)

// LimitAndUsageForProc returns memory limit and usage for cgroup where proc
// is running.
func LimitAndUsageForProc(proc *os.Process) (uint64, uint64, error) {
	limit, err := LimitForProc(proc)
	if err != nil {
		return 0, 0, fmt.Errorf("error reading memory limit for pid %d: %w", proc.Pid, err)
	}
	usage, err := UsageForProc(proc)
	if err != nil {
		return 0, 0, fmt.Errorf("error reading memory usage for pid %d: %w", proc.Pid, err)
	}
	return limit, usage, nil
}

// LimitForProc returns the max memory on process' cgroup. attempts to read using cgroups v1 and
// falls back to v2 if necessary.
func LimitForProc(proc *os.Process) (uint64, error) {
	limitFile := fmt.Sprintf("/proc/%d/root/%s", proc.Pid, limitSuffixPathCgroupV1)
	if val, err := readUint64FromFile(limitFile); err == nil {
		return val, nil
	}
	path, err := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", proc.Pid))
	if err != nil {
		return 0, err
	}
	path = bytes.TrimPrefix(path, []byte("0::"))
	path = bytes.TrimSuffix(path, []byte("\n"))
	spath := fmt.Sprintf("/sys/fs/cgroup/%s/memory.max", string(path))
	return readUint64FromFile(spath)
}

// UsageForProc returns the amount of memory currently in use within the namespace
// where proc lives.
func UsageForProc(proc *os.Process) (uint64, error) {
	usageFile := fmt.Sprintf("/proc/%d/root/%s", proc.Pid, usageSuffixPathCgroupV1)
	if val, err := readUint64FromFile(usageFile); err == nil {
		return val, nil
	}
	path, err := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", proc.Pid))
	if err != nil {
		return 0, err
	}
	path = bytes.TrimPrefix(path, []byte("0::"))
	path = bytes.TrimSuffix(path, []byte("\n"))
	spath := fmt.Sprintf("/sys/fs/cgroup/%s/memory.current", string(path))
	return readUint64FromFile(spath)
}

func readUint64FromFile(fpath string) (uint64, error) {
	contentAsB, err := ioutil.ReadFile(fpath)
	if err != nil {
		return 0, err
	}
	contentAsStr := strings.TrimSuffix(string(contentAsB), "\n")
	return strconv.ParseUint(contentAsStr, 10, 64)
}
