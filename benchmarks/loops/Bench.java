public class Bench {
    public static long loop_sum() {
        long sum = 0;
        long i = 0;
        while (i < 10000000) {
            sum += i;
            i++;
        }
        return sum;
    }
    public static void main(String[] args) {
        System.out.println(loop_sum());
    }
}
