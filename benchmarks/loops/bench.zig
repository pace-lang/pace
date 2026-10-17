const std = @import("std");
fn loop_sum() i64 {
    var sum: i64 = 0;
    var i: i64 = 0;
    while (i < 10000000) {
        sum += i;
        i += 1;
    }
    return sum;
}
pub fn main() void {
    std.debug.print("{d}\n", .{loop_sum()});
}
