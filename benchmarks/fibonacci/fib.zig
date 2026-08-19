const std = @import("std");

fn fib(n: i32) i32 {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

pub fn main() !void {
    std.debug.print("Calculating fib(35)...\n", .{});
    const result = fib(35);
    std.debug.print("Result:\n{d}\n", .{result});
}
