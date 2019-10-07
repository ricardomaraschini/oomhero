// XXX
// This is a sample application that allocates 512kb of memory per second,
// this is the application we use on our documentation.
package main

import (
	"fmt"
	"os"
	"os/signal"
	"syscall"
	"time"
)

var (
	// This grows indefinetely.
	bloat []int8
)

func main() {
	sigs := make(chan os.Signal, 1)

	// SIGTERM would be send by K8s on eviction, we wont receive it
	// here because we are going to die of memory starvation. SIGUSR1
	// will be send by oomhero once we cross the Warning threshold and
	// SIGUSR2 will be send when we cross the configured Critical.
	signal.Notify(
		sigs,
		syscall.SIGTERM,
		syscall.SIGUSR1,
		syscall.SIGUSR2,
	)

	// start bloating our process memory.
	go leak()

	for {
		// print all received signals.
		sig := <-sigs
		switch sig {
		case syscall.SIGUSR1:
			// here we could shrink our memory usage, maybe
			// by cleaning up caches.
			fmt.Println("warning level reached")
		case syscall.SIGUSR2:
			// if we are over Critical we should set ourselves
			// as unhealthy so K8S would send us a SIGTERM and
			// give us some room to graceful shutdown.
			fmt.Println("critical level reached")
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
