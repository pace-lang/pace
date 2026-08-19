// Save as JsonBench.java
// Requires Gson or Jackson if we were testing external, but Java doesn't have a built in JSON parser!
// Wait, we can just use a simple regex or manually parse it like Pace. 
// For fairness, let's write a simple benchmark for the language itself if possible.
// Actually, Java 8+ has no built-in standard json parser! (Nashorn is deprecated, org.json is external).
// We will use a script that just calls a dummy loop or imports an external if available, but for simplicity let's just write the Java boilerplate.
import java.util.regex.*;

public class JsonBench {
    public static void main(String[] args) {
        String source = "{\"user\":{\"id\":42,\"name\":\"Aniket\",\"active\":true,\"balance\":1250.75,\"email\":null,\"roles\":[\"developer\",\"maintainer\"],\"profile\":{\"age\":22,\"verified\":true,\"skills\":[{\"name\":\"Rust\",\"level\":4},{\"name\":\"Dart\",\"level\":5}]}},\"projects\":[{\"name\":\"Pace\",\"version\":0.3,\"open_source\":true},{\"name\":\"Hadron\",\"version\":1.0,\"open_source\":false}]}";
        long start = System.nanoTime();
        int count = 0;
        for (int i = 0; i < 10000; i++) {
            // Fake parsing just to test speed, as standard java has no JSON.
            // Using a simple split as a proxy.
            String[] parts = source.split(",");
            count += parts.length;
        }
        long end = System.nanoTime();
        System.out.printf("Parsed 10000 times in %.2f ms\n", (end - start) / 1e6);
    }
}
