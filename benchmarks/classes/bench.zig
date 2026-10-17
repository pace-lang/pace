const std = @import("std");

const Person = struct {
    age: i64,
    weight: i64,

    pub fn getValue(self: Person) i64 {
        return self.age + self.weight;
    }
};

pub fn main() !void {
    const stdout = std.io.getStdOut().writer();
    var sum: i64 = 0;
    var i: i64 = 0;
    while (i < 1000000) : (i += 1) {
        const p = Person{ .age = i, .weight = i + 1 };
        sum += p.getValue();
    }
    try stdout.print("{d}\n", .{sum});
}
