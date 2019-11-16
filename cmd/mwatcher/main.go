package main

import (
	"context"
	"log"

	"github.com/ricardomaraschini/oomhero/mount"
	"github.com/ricardomaraschini/oomhero/proc"

	"github.com/ricardomaraschini/crebain/watcher"
)

func main() {
	ctx, cancel := context.WithCancel(context.Background())

	ps, err := proc.Others()
	if err != nil {
		log.Fatalf("error reading process list: %v", err)
	}

	for _, p := range ps {
		mpoints, err := mount.TMPFSPoints(p)
		if err != nil {
			log.Printf("error reading mountpoints: %s", err)
			continue
		}

		handler := &procHandler{
			proc: p,
			ctx:  ctx,
		}

		for _, mp := range mpoints {
			w, err := watcher.New(mp, nil, handler)
			if err != nil {
				log.Printf("creating watcher: %s", err)
				continue
			}
			w.Loop()
		}
	}

	// XXX FIXME
	defer cancel()
	select {}
}
