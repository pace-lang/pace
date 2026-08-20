package main

import (
    "fmt"
    "net/http"
)

func main() {
    http.HandleFunc("/json", func(w http.ResponseWriter, r *http.Request) {
        w.Header().Set("Content-Type", "application/json")
        fmt.Fprint(w, `{"message":"Hello, World!"}`)
    })
    http.ListenAndServe(":3000", nil)
}
