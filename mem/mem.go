package mem

import (
	"bytes"
	"fmt"
	"io/ioutil"
	"os"
	"strconv"
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
	if val, err := readBytesFromFile(limitFile); err == nil {
		return val, nil
	}
	path, err := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", proc.Pid))
	if err != nil {
		return 0, err
	}
	pbegin := bytes.IndexByte(path, '/')
	if pbegin == -1 {
		return 0, fmt.Errorf("invalid cgroup path: %s", path)
	}
	path = bytes.TrimSpace(path[pbegin:])
	spath := fmt.Sprintf("/sys/fs/cgroup/%s/memory.max", string(path))
	return readBytesFromFile(spath)
}

// UsageForProc returns the amount of memory currently in use within the namespace
// where proc lives.
func UsageForProc(proc *os.Process) (uint64, error) {
	usageFile := fmt.Sprintf("/proc/%d/root/%s", proc.Pid, usageSuffixPathCgroupV1)
	if val, err := readBytesFromFile(usageFile); err == nil {
		return val, nil
	}
	path, err := os.ReadFile(fmt.Sprintf("/proc/%d/cgroup", proc.Pid))
	if err != nil {
		return 0, err
	}
	pbegin := bytes.IndexByte(path, '/')
	if pbegin == -1 {
		return 0, fmt.Errorf("invalid cgroup path: %s", path)
	}
	path = bytes.TrimSpace(path[pbegin:])
	spath := fmt.Sprintf("/sys/fs/cgroup/%s/memory.current", string(path))
	return readBytesFromFile(spath)
}

// readBytesFromFile reads a file and returns its content as a uint64. if the string
// "max" is found, this returns 0.
func readBytesFromFile(fpath string) (uint64, error) {
	content, err := ioutil.ReadFile(fpath)
	if err != nil {
		return 0, err
	}
	content = bytes.TrimSpace(content)
	if string(content) == "max" {
		return 0, nil
	}
	return strconv.ParseUint(string(content), 10, 64)
}
