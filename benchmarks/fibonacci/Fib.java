public class Fib {
    public static int fib(int n) {
        if (n <= 1) return n;
        return fib(n - 1) + fib(n - 2);
    }

    public static void main(String[] args) {
        System.out.println("Calculating fib(35)...");
        int result = fib(35);
        System.out.println("Result:\n" + result);
    }
}
