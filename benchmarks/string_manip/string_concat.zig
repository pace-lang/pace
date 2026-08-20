const std = @import("std");

pub fn main() !void {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    const allocator = arena.allocator();

    var s = std.ArrayList(u8).empty;
    defer s.deinit(allocator);

    var i: usize = 0;
    while (i < 100000) : (i += 1) {
        try s.append(allocator, 'a');
    }

    std.debug.print("{d}\n", .{s.items.len});
}
