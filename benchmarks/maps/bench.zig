const std = @import("std");
fn map_test(allocator: std.mem.Allocator) !i64 {
    var map = std.AutoHashMap(i64, i64).init(allocator);
    defer map.deinit();
    
    var i: i64 = 0;
    while (i < 10000) {
        try map.put(i, i * 2);
        i += 1;
    }
    
    var sum: i64 = 0;
    i = 0;
    while (i < 10000) {
        if (map.get(i)) |val| {
            sum += val;
        }
        i += 1;
    }
    return sum;
}
pub fn main() !void {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    const result = try map_test(arena.allocator());
    std.debug.print("{d}\n", .{result});
}
