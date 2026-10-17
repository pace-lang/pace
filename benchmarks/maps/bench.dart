int map_test() {
  var map = <int, int>{};
  for (int i = 0; i < 10000; i++) {
    map[i] = i * 2;
  }
  int sum = 0;
  for (int i = 0; i < 10000; i++) {
    sum += map[i]!;
  }
  return sum;
}
void main() {
  print(map_test());
}
