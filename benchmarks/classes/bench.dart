class Person {
  int age;
  int weight;
  Person(this.age, this.weight);
  int getValue() => age + weight;
}

void main() {
  int sum = 0;
  for (int i = 0; i < 1000000; i++) {
    var p = Person(i, i + 1);
    sum += p.getValue();
  }
  print(sum);
}
