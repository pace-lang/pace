String concat_test() {
  StringBuffer s = StringBuffer();
  for (int i = 0; i < 10000; i++) {
    s.write("a");
  }
  return s.toString();
}
void main() {
  print(concat_test().length);
}
