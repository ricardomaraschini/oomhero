# OOMHero

OOMHero is a sidecar that helps you to keep track of your containers memory
usage. By implementing it two signals are going to be send to your container as
the memory usage grows: a _warning_ and a _critical_ signals. By leveraging
these signals you might be able to defeat the deadly `OOMKiller`.

## What it does
Runs as a sidecar in pods with `shareProcessNamespace: true`, monitoring memory
usage of all pod processes. Sends signals when processes exceed configurable
watermarks:

- **SIGUSR1** at warning threshold (default: 75%)
- **SIGUSR2** at critical threshold (default: 90%)

Applications handle these signals to take action before the `OOMKiller` strikes
such as flush caches, shed load, dump diagnostics, restart gracefully, etc.

### How it works
This sidecar will send your container two signals: when memory usage crosses
so called _warning_(**SIGUSR1** by default) and _critical_(**SIGUSR2** by default) thresholds.
It is possible to use different signals by specifying appropriate environment variables.
Your application must be able to deal with these signals by implementing
signal handlers.

### On limits
If only `requests` are specified during the pod Deployment no signal will be
sent, this sidecar operates only on `limits`.

### Deployment example

The Pod below is composed by two distinct containers, the first one is called
`bloat` and its purpose is (as the name implies) to simulate a memory leak by
constantly allocating in a global variable. The sidecar is an `OOMHero`
configured to send a `SIGUSR1` (warning) when `bloat` reaches 75% and a
`SIGUSR2` (critical) on 90%. The only pre-requisite is that both containers
share the same process namespace, hence `shareProcessNamespace` is set to
`true`.

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
