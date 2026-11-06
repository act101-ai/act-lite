fn f(a: u8, b: u8, c: u8, d: u8, e: u8, g: u8) u8 {
    var y: u8 = 0;
    if (a > 0 and b > 0) {
        for (0..c) |i| {
            switch (i) {
                1 => {
                    while (y < d) {
                        if (g > 0) {
                            y += 1;
                        }
                    }
                },
                else => y = 3,
            }
        }
    } else if (e > 0) {
        y = 4;
    }
    const z = if (y > 0 or e > 0) y else 0;
    const w = h(z) catch 0;
    const v = for (0..c) |j| j else w;
    const u = while (y < d) : (y += 1) y else v;
    y += u;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    y += 1;
    return y;
}

const S = struct {
    x: u8,
    fn m(self: S, k: u8) u8 {
        return self.x + k;
    }
};
