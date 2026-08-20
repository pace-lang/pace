import java.util.ArrayList;

public class actor_spawn {
    public static void main(String[] args) throws InterruptedException {
        ArrayList<Thread> threads = new ArrayList<>();
        for (int i = 0; i < 10000; i++) {
            Thread t = new Thread(() -> {
                int id = 0;
            });
            threads.add(t);
            t.start();
        }
        for (Thread t : threads) {
            t.join();
        }
        System.out.println("Spawned 10000 actors.");
    }
}
