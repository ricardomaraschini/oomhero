// XXX
// This is a sample application that allocates 512kb of memory per second,
// this is the application we use on our documentation.
package main

import (
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"
)

var (
	// This grows indefinetely.
	bloat []int8
	// This indicates if we are healthy or not.
	unhealthy bool
)

func main() {
	// set up a liveness probe, it will indicate healthy until se set unhealthy
	// variable to true.
	http.HandleFunc("/healthz", func(w http.ResponseWriter, r *http.Request) {
		if unhealthy {
			w.WriteHeader(http.StatusInternalServerError)
			fmt.Println("liveness reporting unhealthy")
			return
		}
		fmt.Println("liveness reporting healthy")
	})
	go http.ListenAndServe(":8080", nil)

	sigs := make(chan os.Signal, 1)

	// SIGTERM will be send by K8s on pod eviction, we should receive it
	// here only after setting the container as unhealthy. SIGUSR1 will
	// be send by OOMHero once we cross the Warning threshold and SIGUSR2
	// will be send when we cross Critical.
	signal.Notify(
		sigs,
		syscall.SIGTERM,
		syscall.SIGUSR1,
		syscall.SIGUSR2,
	)

	// start bloating our process memory.
	go leak()

	for {
		sig := <-sigs
		switch sig {
		case syscall.SIGUSR1:
			// here we could shrink our memory usage, maybe
			// by cleaning up caches.
			fmt.Println("warning level reached")
		case syscall.SIGUSR2:
			// if we are over Critical we should set ourselves
			// as unhealthy so K8S would send us a SIGTERM and
			// give us some room for a graceful shutdown.
			fmt.Println("critical level reached")
			unhealthy = true
		case syscall.SIGTERM:
			// we manage to set ourselves as unhealthy and to
			// receive a SIGTERM directly from K8S.
			fmt.Println("k8s says it is time to die, bye")
			os.Exit(0)
		}
	}
}

func leak() {
	limit := 1 << 20
	for {
		for i := 0; i < limit; i++ {
			bloat = append(bloat, 1)
		}
		time.Sleep(2 * time.Second)
	}
}
