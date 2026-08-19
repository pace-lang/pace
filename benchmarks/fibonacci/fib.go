package main

import "fmt"

func fib(n int) int {
	if n <= 1 {
		return n
	}
	return fib(n-1) + fib(n-2)
}

func main() {
	fmt.Println("Calculating fib(35)...")
	result := fib(35)
	fmt.Printf("Result:\n%d\n", result)
}
