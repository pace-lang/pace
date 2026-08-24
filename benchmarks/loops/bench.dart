int loop_sum() {
  int sum = 0;
  int i = 0;
  while (i < 10000000) {
    sum += i;
    i++;
  }
  return sum;
}
void main() {
  print(loop_sum());
}
