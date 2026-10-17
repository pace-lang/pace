public class Bench {
    public static String concat_test() {
        StringBuilder s = new StringBuilder();
        for (int i = 0; i < 10000; i++) {
            s.append("a");
        }
        return s.toString();
    }
    public static void main(String[] args) {
        System.out.println(concat_test().length());
    }
}
