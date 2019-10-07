package mem

import (
	"io/ioutil"
	"strconv"
	"strings"
)

var (
	limitFile = "/sys/fs/cgroup/memory/memory.limit_in_bytes"
	usageFile = "/sys/fs/cgroup/memory/memory.usage_in_bytes"
)

// Limit returns the max memory according to what is reported by process
// memory cgroup.
func Limit() (uint64, error) {
	limitAsB, err := ioutil.ReadFile(limitFile)
	if err != nil {
		return 0, err
	}
	limitAsStr := strings.TrimSuffix(string(limitAsB), "\n")
	return strconv.ParseUint(limitAsStr, 10, 64)
}

// Usage returns the amount of memory currently in usage as reported by
// process memory cgroup.
func Usage() (uint64, error) {
	usageAsB, err := ioutil.ReadFile(usageFile)
	if err != nil {
		return 0, err
	}
	usageAsStr := strings.TrimSuffix(string(usageAsB), "\n")
	return strconv.ParseUint(usageAsStr, 10, 64)
}
