# oomhero
Memory monitoring sidecar for Kubernetes pods. Watches process memory usage and
sends signals when thresholds are crossed.

## What it does
Runs as a sidecar in pods with `shareProcessNamespace: true`, monitoring memory
usage of all pod processes. Sends signals when processes exceed configurable
watermarks:

- **SIGUSR1** at warning threshold (default: 75%)
- **SIGUSR2** at critical threshold (default: 90%)

Applications handle these signals to take action before the `OOMKiller` strikes
- flush caches, shed load, dump diagnostics, restart gracefully.

## Why
The Linux `OOMKiller` gives no warning before SIGKILL. oomhero provides early
warning so applications can respond proactively to memory pressure.

## Usage
```bash
oomhero --warning 75 --critical 90 --interval 10ms
```
**Options:**
- `--warning <N>` - Warning threshold % (default: 75)
- `--critical <N>` - Critical threshold % (default: 90)
- `--interval <DURATION>` - Scan interval (default: 10ms)

**Environment:**
- `RUST_LOG=info` - Log level (debug, info, warn, error)

## Deployment example

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-bloating-app
spec:
  shareProcessNamespace: true
  containers:
  - name: bloat
    image: quay.io/rmarasch/bloat:latest
    livenessProbe:
      periodSeconds: 3
      failureThreshold: 1
      httpGet:
        path: /healthz
        port: 8080
    resources:
      requests:
        memory: "256Mi"
        cpu: "250m"
      limits:
        memory: "512Mi"
        cpu: "250m"
  - name: oomhero
    image: docker.io/ricardomaraschini/oomhero:v2
    env:
    - name: RUST_LOG
      value: info
    resources:
      limits:
        cpu: "1m"
        memory: "32Mi"
    securityContext:
      capabilities:
        add:
        - SYS_PTRACE
```

## How it works

1. Detects cgroups version (v1 or v2).
2. Lists all processes in `/proc`.
3. Reads memory usage from cgroup files per process.
4. Calculates usage % against memory limit.
5. Sends signals when thresholds are crossed.

> [!IMPORTANT]
> The scan loop runs continuously at `--interval` rate, the default is to run a
> full scan every 10ms making it **very** aggressive with regards to CPU usage,
> this is by design. You can adjust the interval by setting proper
> `resource.limits.cpu` values for the container **OR** by passing a custom
> `--interval` flag.

## Requirements

- Kubernetes pod with `shareProcessNamespace: true`.
- `SYS_PTRACE` capability enabled.
- Processes must have memory limits set.

## Building

```bash
$ make build
```
```
$ podman build -t my-registry.com/namespace/oomhero:latest -f Containerfile .
```
