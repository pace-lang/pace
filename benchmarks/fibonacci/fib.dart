int fib(int n) {
  if (n <= 1) return n;
  return fib(n - 1) + fib(n - 2);
}

void main() {
  print("Calculating fib(35)...");
  int result = fib(35);
  print("Result:\n$result");
}
