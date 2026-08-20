const std = @import("std");

fn worker(id: usize) void {
    _ = id;
}

pub fn main() !void {
    var threads: [10000]std.Thread = undefined;
    var i: usize = 0;
    while (i < 10000) : (i += 1) {
        threads[i] = try std.Thread.spawn(.{}, worker, .{i});
    }

    i = 0;
    while (i < 10000) : (i += 1) {
        threads[i].join();
    }

    std.debug.print("Spawned 10000 actors.\n", .{});
}
