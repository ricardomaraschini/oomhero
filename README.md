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
This sidecar will send your container two signals: when memory usage crosses so
called _warning_(**SIGUSR1** by default) and _critical_(**SIGUSR2** by default)
thresholds. It is possible to use different signals by specifying appropriate
command line flags.
Your application must be able to deal with these signals by implementing signal
handlers.

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
oomhero --warning 75 --critical 90 --interval 200ms

```

## Options:
```
--warning <WARNING>                  Warning memory usage watermark [default: 75]
--critical <CRITICAL>                Critical memory usage watermark [default: 90]
--interval <INTERVAL>                How often scan all processes [default: 100ms]
--cooldown <COOLDOWN>                Interval between signals [default: 30s]
--warning-signal <WARNING_SIGNAL>    Signal send on warning [default: SIGUSR1]
--critical-signal <CRITICAL_SIGNAL>  Signal send on critical [default: SIGUSR2]
--version                            Print version
--verbose                            Set logging to verbose
--help                               Print help
```
