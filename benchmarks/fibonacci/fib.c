#include <stdio.h>

int fib(int n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

int main() {
    printf("Calculating fib(35)...\n");
    int result = fib(35);
    printf("Result:\n%d\n", result);
    return 0;
}
