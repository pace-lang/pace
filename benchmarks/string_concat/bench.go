package main
import (
    "fmt"
    "strings"
)
func concat_test() string {
    var s strings.Builder
    for i := 0; i < 10000; i++ {
        s.WriteString("a")
    }
    return s.String()
}
func main() {
    fmt.Println(len(concat_test()))
}
