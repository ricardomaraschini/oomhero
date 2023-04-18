package main

import (
	"log"
	"os"
	"strconv"

	"github.com/ricardomaraschini/oomhero/proc"
)

var (
	warning  uint64 = 75
	critical uint64 = 90
	cooldown uint64 = 60
)

// reads warning and critical from environment or use the default ones.
func init() {
	warningEnv := envVarToUint64("WARNING", warning)
	criticalEnv := envVarToUint64("CRITICAL", critical)
	cooldownEnv := envVarToUint64("COOLDOWN", cooldown)

	if warningEnv > 100 || criticalEnv > 100 {
		log.Print("warning and critical must be lower or equal to 100")
		return
	} else if warningEnv > criticalEnv {
		log.Print("warning must be lower or equal to critical")
		return
	}

	warning = warningEnv
	critical = criticalEnv
	cooldown = cooldownEnv
}

// envVarToUint64 converts the environment variable into a uint64, in case of
// error provided default value(def) is returned instead.
func envVarToUint64(name string, def uint64) uint64 {
	asString := os.Getenv(name)
	if asString == "" {
		return def
	}

	val, err := strconv.ParseUint(asString, 10, 64)
	if err != nil {
		return def
	}

	return val
}

func getOsProcesses() ([]proc.Process, error) {
	ps, err := proc.Others()
	if err != nil {
		return nil, err
	}

	osps := make([]proc.Process, len(ps))
	for _, p := range ps {
		osps = append(osps, proc.NewOsProcess(p))
	}

	return osps, nil
}

func main() {
	log.Printf("warning threshold set to %d%%", warning)
	log.Printf("critical threshold set to %d%%", critical)

	watchProcesses(getOsProcesses)
}
