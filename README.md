# OOMHero

A lightweight Kubernetes sidecar that monitors process resource usage and
pressure metrics, sending configurable signals to applications before resource
exhaustion occurs.

> [!IMPORTANT]
> **Breaking Change (v2.x+):** OOMHero has moved to an expression-based threshold
> system. The previous specific flags (e.g., `--memory-usage-warning`) have been
> removed. You must now use the `--warning` and `--critical` flags with
> expressions. See [Threshold Expressions](#threshold-expressions) for details.

## Overview

OOMHero runs alongside your application containers in Kubernetes pods,
continuously monitoring memory usage, memory pressure, I/O pressure, and CPU
pressure. When processes cross configurable thresholds (defined as
expressions), OOMHero sends Unix signals to enable proactive remediation before
the OOMKiller terminates your application.

## Features

- **Expression-based thresholds**: Define complex triggers using any combination of
  memory, OOM score, and pressure metrics
- **Signal-based notifications**: Sends customizable Unix signals (default:
  `SIGUSR1` for warning, `SIGUSR2` for critical)
- **HTTP notifications**: Send alerts via HTTP POST requests instead of Unix signals
- **Cooldown periods**: Prevents signal spam with configurable intervals
  between notifications
- **Low overhead**: Minimal resource footprint (typically 1m CPU, 32Mi memory)

## How It Works

OOMHero operates in pods with `shareProcessNamespace: true`, enabling it to
monitor all processes within the pod. It continuously scans processes at
configurable intervals, evaluating their resource usage against defined
threshold expressions.

When a process matches an expression:
1. **Warning expression**: Sends SIGUSR1 (or custom signal) to the process
2. **Critical expression**: Sends SIGUSR2 (or custom signal) to the process

Applications implement signal handlers to take corrective action such as:
- Flushing caches to disk
- Shedding non-critical workloads
- Triggering graceful degradation
- Dumping diagnostics for post-mortem analysis
- Initiating controlled restarts

### Threshold Expressions

OOMHero uses the [fasteval](https://github.com/likebike/fasteval) library to
evaluate threshold expressions. You can combine various metrics using standard
operators:

- **Logical**: `&&` (and), `||` (or), `!` (not)
- **Comparison**: `>`, `<`, `>=`, `<=`, `==`, `!=`
- **Algebraic**: `+`, `-`, `*`, `/`, `%` (modulo), `^` (power)

#### Available Variables

| Variable | Type | Description |
|----------|------|-------------|
| `memory_usage` | `f64` | Current memory usage as a percentage of the limit (%) |
| `memory_current` | `f64` | Current memory usage in bytes |
| `memory_max` | `f64` | Memory limit in bytes |
| `oom_score` | `f64` | Current OOM score |
| `oom_score_adj` | `f64` | OOM score adjustment |
| `{resource}_pressure_{severity}_{window}` | `f64` | Pressure metrics |

**Pressure Metric Components:**
- **Resource**: `memory`, `io`, `cpu`
- **Severity**: `some`, `full`
- **Window**: `avg10`, `avg60`, `avg300`, `total`

*Example*: `memory_pressure_full_avg10 > 20`

## Metrics

OOMHero exposes Prometheus metrics on port `9000` by default. These metrics provide
real-time visibility into the resource usage and pressure of all processes being
monitored.

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `memory_usage` | Gauge | `pid`, `cmdline` | Current memory usage as a percentage of the limit |
| `oom_score` | Gauge | `pid`, `cmdline` | Current OOM score (including adjustment) |
| `memory_pressure` | Gauge | `pid`, `cmdline`, `severity_level`, `severity_window` | Memory pressure stall information |
| `io_pressure` | Gauge | `pid`, `cmdline`, `severity_level`, `severity_window` | I/O pressure stall information |
| `cpu_pressure` | Gauge | `pid`, `cmdline`, `severity_level`, `severity_window` | CPU pressure stall information |

### Metric Labels

- `pid`: Process ID
- `cmdline`: The command line of the process
- `severity_level`: Either `some` or `full`
- `severity_window`: One of `avg10`, `avg60`, `avg300`, or `total`

Metrics have an idle timeout of 1 minute; if a process (identified by pid and
cmdline) is not seen for 1 minute, its metrics will be removed.

## Requirements

- Kubernetes cluster with Linux nodes (kernel 4.20+ for full PSI support)
- Pod must have `shareProcessNamespace: true`
- Container requires `SYS_PTRACE` capability to send signals
- Both `--warning` and `--critical` expressions have default values but
  they should be be customized by the user.

## Installation

### Using Pre-built Container

> [!NOTE]
> On https://github.com/ricardomaraschini/oomhero/pkgs/container/oomhero
> you can find what is the last stable release of the container image. The
> example below uses `latest` but that should not be used.

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
    image: ghcr.io/ricardomaraschini/oomhero:latest
    args:
    - "--warning=memory_usage > 75"
    - "--critical=memory_usage > 90"
    - "--loop-interval=100ms"
    - "--cooldown-interval=30s"
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
./target/release/oomhero --warning "memory_usage > 75" --critical "memory_usage > 90"
```

## Usage

### Basic Memory Monitoring

```bash
oomhero \
  --warning "memory_usage > 75" \
  --critical "memory_usage > 90" \
  --loop-interval 100ms \
  --cooldown-interval 30s
```

### Comprehensive Resource Monitoring

```bash
oomhero \
  --warning "memory_usage > 70 || memory_pressure_full_avg60 > 50" \
  --critical "memory_usage > 85 || memory_pressure_full_avg60 > 80" \
  --loop-interval 200ms \
  --cooldown-interval 30s
```

### Custom OOM Score Logic

```bash
oomhero \
  --warning "oom_score > 500" \
  --critical "oom_score > 800"
```

### Custom Signals

```bash
oomhero \
  --warning "memory_usage > 75" \
  --critical "memory_usage > 90" \
  --warning-signal SIGHUP \
  --critical-signal SIGTERM
```

### HTTP Notifications

```bash
oomhero \
  --warning "memory_usage > 75" \
  --critical "memory_usage > 90" \
  --http-file-path /etc/oomhero/config.yaml
```

**Config file format** (`config.yaml`):
```yaml
url: https://hooks.example.com/alerts
headers:
  - name: Authorization
    value: Bearer token123
  - name: Content-Type
    value: application/json
```

**HTTP request body**:
```json
{
  "severity": "Warning",
  "process": {
    "pid": 1234,
    "cmdline": "/usr/bin/myapp"
  },
  "collected_data": {
    "memory_max": 536870912,
    "memory_current": 421527552,
    "memory_usage": 78.5,
    "oom_score": 250,
    "oom_score_adj": 0,
    "pressure": {
      "memory": {
        "some": {"avg10": 5.2, "avg60": 3.1, "avg300": 2.8, "total": 1500000},
        "full": {"avg10": 0.0, "avg60": 0.0, "avg300": 0.0, "total": 0}
      },
      "io": {
        "some": {"avg10": 0.0, "avg60": 0.0, "avg300": 0.0, "total": 0},
        "full": {"avg10": 0.0, "avg60": 0.0, "avg300": 0.0, "total": 0}
      },
      "cpu": {
        "some": {"avg10": 0.0, "avg60": 0.0, "avg300": 0.0, "total": 0},
        "full": {"avg10": 0.0, "avg60": 0.0, "avg300": 0.0, "total": 0}
      }
    }
  }
}
```

## Configuration Options

| Option | Description | Default |
|--------|-------------|---------|
| `--warning` | Expression for warning signal | (empty) |
| `--critical` | Expression for critical signal | (empty) |
| `--loop-interval` | Process scanning frequency | 100ms |
| `--cooldown-interval` | Minimum time between repeated signals | 30s |
| `--warning-signal` | Signal sent at warning threshold | SIGUSR1 |
| `--critical-signal` | Signal sent at critical threshold | SIGUSR2 |
| `--http-file-path` | Path to HTTP notification config (conflicts with signal options) | (none) |
| `--version` | Display version information | false |

**Note**: Both `--warning` and `--critical` expressions must be provided for
OOMHero to run.

## Important Considerations

### Memory Limits vs Requests

OOMHero operates based on container **limits**, not requests. If only resource
requests are specified without limits, OOMHero cannot calculate meaningful
usage percentages.

### Performance Impact

OOMHero scans all processes at the configured interval. Use CPU limits to
control scan frequency and resource consumption.

## Troubleshooting

### OOMHero exits with "invalid expression: ..."

Ensure both `--warning` and `--critical` expressions are valid `fasteval`
expressions and provided when starting OOMHero. Example:
```bash
--warning "memory_usage > 75" --critical "memory_usage > 90"
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
