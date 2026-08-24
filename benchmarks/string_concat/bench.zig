const std = @import("std");
fn concat_test(allocator: std.mem.Allocator) ![]u8 {
    var s = std.ArrayList(u8).init(allocator);
    defer s.deinit();
    var i: usize = 0;
    while (i < 10000) {
        try s.appendSlice("a");
        i += 1;
    }
    return try s.toOwnedSlice();
}
pub fn main() !void {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    const result = try concat_test(arena.allocator());
    std.debug.print("{d}\n", .{result.len});
}
