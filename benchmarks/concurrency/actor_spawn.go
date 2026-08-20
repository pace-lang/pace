package main

import (
	"fmt"
	"sync"
)

func main() {
	var wg sync.WaitGroup
	for i := 0; i < 10000; i++ {
		wg.Add(1)
		go func(id int) {
			_ = id
			wg.Done()
		}(i)
	}
	wg.Wait()
	fmt.Println("Spawned 10000 actors.")
}
