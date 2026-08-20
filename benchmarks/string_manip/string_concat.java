public class string_concat {
    public static void main(String[] args) {
        String s = "";
        for (int i = 0; i < 100000; i++) {
            s += "a";
        }
        System.out.println(s.length());
    }
}
