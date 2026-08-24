package main
import "fmt"
func map_test() int64 {
    m := make(map[int64]int64)
    var i int64
    for i = 0; i < 10000; i++ {
        m[i] = i * 2
    }
    var sum int64 = 0
    for i = 0; i < 10000; i++ {
        sum += m[i]
    }
    return sum
}
func main() {
    fmt.Println(map_test())
}
