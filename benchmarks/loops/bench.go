package main
import "fmt"
func loop_sum() int64 {
    var sum int64 = 0
    var i int64 = 0
    for i < 10000000 {
        sum += i
        i++
    }
    return sum
}
func main() {
    fmt.Println(loop_sum())
}
