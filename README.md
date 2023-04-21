# OOMHero

OOMHero is a sidecar that helps you to keep track of your containers memory
usage. By implementing it two signals are going to be send to your container
as the memory usage grows: a _warning_ and a _critical_ signals. By leveraging
these signals you might be able to defeat the deadly `OOMKiller`.

### How it works

This sidecar will send your container two signals: when memory usage crosses
so called _warning_(**SIGUSR1** by default) and _critical_(**SIGUSR2** by default) thresholds. 
It is possible to use different signals by specifying appropriate environment variables.
Your application must be able to deal with these signals by implementing
signal handlers.

You an see [here](https://github.com/ricardomaraschini/oomhero/blob/master/cmd/bloat/main.go)
an example of how to capture the signals in Go.

### On limits

If only `requests` are specified during the pod Deployment no signal will be
sent, this sidecar operates only on `limits`.

### Deployment example

The Pod below is composed by two distinct containers, the first one is called
`bloat` and its purpose is(as the name implies) to simulate a memory leak by
constantly allocating in a global variable. The sidecar is an `OOMHero` 
configured to send a `SIGUSR1`(warning) when `bloat` reaches 65% and a `SIGUSR2`
(critical) on 90%. The only pre-requisite is that both containers share the same
process namespace, hence `shareProcessNamespace` is set to `true`.


```yaml
apiVersion: v1
kind: Pod
metadata:
  name: oomhero
spec:
  shareProcessNamespace: true
  containers:
    - name: bloat
      image: quay.io/rmarasch/bloat:latest
      imagePullPolicy: Always
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
          memory: "256Mi"
          cpu: "250m"
    - name: oomhero
      image: quay.io/rmarasch/oomhero:latest
      imagePullPolicy: Always
      securityContext:
        privileged: true
      env:
      - name: WARNING
        value: "65"
      - name: CRITICAL
        value: "90" 
```

Saving the above yaml into a file you just need to deploy it:

```bash
$ kubectl create -f ./pod.yaml
```

That will create a Pod with two containers, you may follow the memory consumption
and signals being sent by inspecting all pod logs.

```bash
$ # for bloat container log
$ kubectl logs -f oomhero --container bloat
$ # for oomhero container log
$ kubectl logs -f oomhero --container oomhero 
```

### Configuring signals
Signals supported by `OOMHero` are:
- SIGABRT
- SIGCONT
- SIGHUP
- SIGINT
- SIGIOT
- SIGKILL
- SIGQUIT
- SIGSTOP
- SIGTERM
- SIGTSTP
- SIGUSR1
- SIGUSR2

To use any of those signals instead of default ones, set `WARNING_SIGNAL` and `CRITICAL_SIGNAL`
environment variable to specify _warning_ and _critical_ signals respectively.
If those environment variables are not set, `OOMHero` will use default values (SIGUSR1 and SIGUSR2).

For instance to send `SIGTERM` when critical threshold is reached put following in pod or deployment definition:

```yaml
containers:
  # other containers omitted for brevity
  - name: oomhero
    image: quay.io/rmarasch/oomhero
    imagePullPolicy: Always
    env:
    - name: WARNING
      value: "65"
    - name: CRITICAL
      value: "90"
    - name: CRITICAL_SIGNAL
      value: "SIGTERM"
```

### Cooldown

By default `OOMHero` sends one signal per second to other processes once they reach warning or critical threshold.
This might be undesireable behavior in some circumstances, therefore cooldown can be configured.
Once set, signal will be sent no more often than once in `cooldown` for each signal type separately.
In other words other processes would not receive more than one warning and one ciritcal signal more often than once in `cooldown`.

To configure cooldown set `COOLDOWN` environment variable in deployment definition to a desired number of seconds:
```yaml
containers:
  # other containers omitted for brevity
  - name: oomhero
    image: quay.io/rmarasch/oomhero
    imagePullPolicy: Always
    env:
    - name: COOLDOWN
      # cooldown's unit is seconds
      value: "60"
```

### Help needed

[Official documentation](https://kubernetes.io/docs/tasks/configure-pod-container/share-process-namespace/)
states that `SYS_PTRACE` capability is mandatory when signaling between containers
on the same Pod. I could not validate if this is true as it works without it on my
K8S cluster. If to make it work you had to add this capability please let me know.
