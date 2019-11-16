package mount

import (
	"bufio"
	"fmt"
	"os"
	"path"
	"strings"
)

// TMPFSPoints returns all tmpfs mount points for a process.
func TMPFSPoints(proc *os.Process) ([]string, error) {
	fmounts := fmt.Sprintf(
		"/proc/%d/root/proc/mounts",
		proc.Pid,
	)

	fp, err := os.Open(fmounts)
	if err != nil {
		return nil, err
	}
	defer fp.Close()

	mpoints := make([]string, 0)
	scanner := bufio.NewScanner(fp)
	for scanner.Scan() {
		line := scanner.Text()
		if !strings.HasPrefix(line, "tmpfs") {
			continue
		}

		words := strings.Fields(line)
		if len(words) < 2 {
			return nil, fmt.Errorf("invalid mount: %q", line)
		}

		dir := fmt.Sprintf("/proc/%d/root/%s", proc.Pid, words[1])
		mpoints = append(mpoints, path.Clean(dir))
	}

	return mpoints, nil
}
