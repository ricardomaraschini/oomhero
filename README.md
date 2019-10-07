# OOMHero

OOMHero sends your container a signal if it is using too much memory or is
about to be **OOMKilled**.

### What

This application is supposed to be deployed as a sidecar and its purpose is to
actively monitor how much memory other containers within the same pod are using.
If the usage goes over pre-defined thresholds, signals are sent in an attempt to
avoid an **OOMKill** by the Kernel.

### Deployment example

The POD below is composed by two distinct containers, the first one is called
`bloat` and its purpose is(as the name implies) to simulate a memory leak by
constantly allocating in a global variable. The second one is an `OOMHero`
configured to send a `SIGUSR1` when `bloat` reaches 65% and a `SIGUSR2` on 90%.
The only pre-req here is that both containers share the same process namespace,
hence `shareProcessNamespace` is set to `true`.

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: oomhero
spec:
  shareProcessNamespace: true
  containers:
    - name: bloat
      image: quay.io/rmarasch/bloat
      imagePullPolicy: Always
      resources:
        requests:
          memory: "64Mi"
          cpu: "250m"
        limits:
          memory: "64Mi"
          cpu: "250m"
    - name: oomhero
      image: quay.io/rmarasch/oomhero
      imagePullPolicy: Always
      env:
      - name: WARNING
        value: "65"
      - name: CRITICAL
        value: "90" 
```

Saving the above yaml into a file you just need to deploy it:

```
$ kubectl create -f ./pod.yaml
```

That will create a Pod with two containers, you may follow the memory consumption
and signals being sent by inspecting all pod logs.

### Help needed

[Official documentation](https://kubernetes.io/docs/tasks/configure-pod-container/share-process-namespace/)
seems to state that `SYS_PTRACE` capability is mandatory when signaling between
containers on the same Pod. I could not validate if this is true as it works
without it on my K8S cluster. If to make it work you had to add this capability
please let me know.
