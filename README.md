# OOMHero

A lightweight Kubernetes sidecar that monitors process resource usage and
pressure metrics, sending configurable signals to applications before resource
exhaustion occurs.

## Overview

OOMHero runs alongside your application containers in Kubernetes pods,
continuously monitoring memory usage, memory pressure, I/O pressure, and CPU
pressure. When processes approach configurable thresholds, OOMHero sends Unix
signals to enable proactive remediation before the OOMKiller terminates your
application.

## Features

- **Multi-metric monitoring**: Tracks memory usage, memory pressure (PSI), I/O
  pressure, and CPU pressure
- **Signal-based notifications**: Sends customizable Unix signals (default:
  `SIGUSR1` for warning, `SIGUSR2` for critical)
- **Cooldown periods**: Prevents signal spam with configurable intervals
  between notifications
- **Low overhead**: Minimal resource footprint (typically 1m CPU, 32Mi memory)

## How It Works

OOMHero operates in pods with `shareProcessNamespace: true`, enabling it to
monitor all processes within the pod. It continuously scans processes at
configurable intervals, evaluating their resource usage against defined
thresholds.

When a process exceeds a threshold:
1. **Warning threshold**: Sends SIGUSR1 (or custom signal) to the process
2. **Critical threshold**: Sends SIGUSR2 (or custom signal) to the process

Applications implement signal handlers to take corrective action such as:
- Flushing caches to disk
- Shedding non-critical workloads
- Triggering graceful degradation
- Dumping diagnostics for post-mortem analysis
- Initiating controlled restarts

### Pressure Stall Information (PSI)

OOMHero leverages Linux PSI metrics to detect resource contention:
- **Memory pressure**: Indicates when processes are waiting for memory
- **I/O pressure**: Detects when processes are blocked on I/O operations
- **CPU pressure**: Identifies when processes cannot get CPU time

These metrics provide early warning of resource saturation before hard limits
are hit.

## Requirements

- Kubernetes cluster with Linux nodes (kernel 4.20+ for full PSI support)
- Pod must have `shareProcessNamespace: true`
- Container requires `SYS_PTRACE` capability to send signals
- At least one pair of warning/critical thresholds must be configured

## Installation

### Using Pre-built Container

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-application
spec:
  shareProcessNamespace: true
  containers:
  - name: app
    image: your-app:latest
    resources:
      limits:
        memory: "512Mi"
        cpu: "500m"
  - name: oomhero
    image: docker.io/ricardomaraschini/oomhero:v2
    args:
      - --memory-usage-warning=75
      - --memory-usage-critical=90
      - --loop-interval=100ms
      - --cooldown-interval=30s
    resources:
      limits:
        cpu: "1m"
        memory: "32Mi"
    securityContext:
      capabilities:
        add:
        - SYS_PTRACE
```

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/oomhero
cd oomhero

# Build release binary
make release

# Run locally
./target/release/oomhero --memory-usage-warning=75 --memory-usage-critical=90
```

## Usage

### Basic Memory Monitoring

```bash
oomhero \
  --memory-usage-warning=75 \
  --memory-usage-critical=90 \
  --loop-interval=100ms \
  --cooldown-interval=30s
```

### Comprehensive Resource Monitoring

```bash
oomhero \
  --memory-usage-warning=70 \
  --memory-usage-critical=85 \
  --memory-pressure-warning=50 \
  --memory-pressure-critical=80 \
  --io-pressure-warning=60 \
  --io-pressure-critical=90 \
  --cpu-pressure-warning=70 \
  --cpu-pressure-critical=95 \
  --loop-interval=200ms \
  --cooldown-interval=30s
```

### Custom Signals

```bash
oomhero \
  --memory-usage-warning=75 \
  --memory-usage-critical=90 \
  --warning-signal=SIGHUP \
  --critical-signal=SIGTERM
```

## Configuration Options

| Option | Description | Default |
|--------|-------------|---------|
| `--memory-usage-warning` | Warning threshold for memory usage (%) | 0 (disabled) |
| `--memory-usage-critical` | Critical threshold for memory usage (%) | 0 (disabled) |
| `--memory-pressure-warning` | Warning threshold for memory pressure (%) | 0 (disabled) |
| `--memory-pressure-critical` | Critical threshold for memory pressure (%) | 0 (disabled) |
| `--io-pressure-warning` | Warning threshold for I/O pressure (%) | 0 (disabled) |
| `--io-pressure-critical` | Critical threshold for I/O pressure (%) | 0 (disabled) |
| `--cpu-pressure-warning` | Warning threshold for CPU pressure (%) | 0 (disabled) |
| `--cpu-pressure-critical` | Critical threshold for CPU pressure (%) | 0 (disabled) |
| `--loop-interval` | Process scanning frequency | 100ms |
| `--cooldown-interval` | Minimum time between repeated signals | 30s |
| `--warning-signal` | Signal sent at warning threshold | SIGUSR1 |
| `--critical-signal` | Signal sent at critical threshold | SIGUSR2 |
| `--version` | Display version information | false |

**Note**: At least one pair of warning and critical thresholds must be
configured for OOMHero to run.

## Important Considerations

### Memory Limits vs Requests

OOMHero operates based on container **limits**, not requests. If only resource
requests are specified without limits, OOMHero cannot calculate meaningful
usage percentages.

### Performance Impact

OOMHero scans all processes at the configured interval. Use CPU limits to
control scan frequency and resource consumption.

## Troubleshooting

### OOMHero exits with "missing warning and critical for at least one specific counter"

Ensure at least one pair of warning and critical thresholds is configured.
Example:
```bash
--memory-usage-warning=75 --memory-usage-critical=90
```

### Signals not being received by application

1. Verify `shareProcessNamespace: true` is set on the pod
2. Confirm OOMHero has `SYS_PTRACE` capability
3. Check application has signal handlers registered
4. Review OOMHero logs for signal delivery errors

### High CPU usage

Reduce scan frequency by increasing `--loop-interval` or set lower CPU limits
to throttle OOMHero's execution rate.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for
details.
