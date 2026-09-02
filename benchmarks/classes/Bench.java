public class Bench {
    static class Person {
        int age;
        int weight;
        Person(int age, int weight) {
            this.age = age;
            this.weight = weight;
        }
        int getValue() {
            return age + weight;
        }
    }

    public static void main(String[] args) {
        long sum = 0;
        for (int i = 0; i < 1000000; i++) {
            Person p = new Person(i, i + 1);
            sum += p.getValue();
        }
        System.out.println(sum);
    }
}
