package main

import "fmt"

type Person struct {
	age    int
	weight int
}

func (p *Person) getValue() int {
	return p.age + p.weight
}

func main() {
	sum := 0
	for i := 0; i < 1000000; i++ {
		p := Person{age: i, weight: i + 1}
		sum += p.getValue()
	}
	fmt.Println(sum)
}
