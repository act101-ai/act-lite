const std = @import("std");
pub const Point = struct {
    x: i32,
    pub fn len(self: Point) i32 {
        return self.x;
    }
};
const Color = enum { red, green };
var counter: u32 = 0;
pub fn main() void {
    if (counter > 0) {} else {}
    while (counter < 3) : (counter += 1) {}
    for ([_]u8{1}) |b| { _ = b; }
    switch (counter) { 0 => {}, else => {} }
}
test "point len" {
    try std.testing.expect(true);
}
