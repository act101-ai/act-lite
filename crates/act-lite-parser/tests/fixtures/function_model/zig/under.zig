fn f(a: u8, b: u8, c: u8, d: u8, e: u8) u8 {
    var y: u8 = 0;
    if (a > 0 and b > 0) {
        for (0..c) |i| {
            switch (i) {
                1 => {
                    while (y < d) {
                        y += 1;
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
    y += w;
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
    y += 1;
    y += 1;
    y += 1;
    return y;
}
