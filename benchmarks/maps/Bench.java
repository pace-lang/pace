import java.util.HashMap;
public class Bench {
    public static long map_test() {
        HashMap<Long, Long> map = new HashMap<>();
        for (long i = 0; i < 10000; i++) {
            map.put(i, i * 2);
        }
        long sum = 0;
        for (long i = 0; i < 10000; i++) {
            sum += map.get(i);
        }
        return sum;
    }
    public static void main(String[] args) {
        System.out.println(map_test());
    }
}
