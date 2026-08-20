package main

import "fmt"

func main() {
	s := ""
	for i := 0; i < 100000; i++ {
		s += "a"
	}
	fmt.Println(len(s))
}
