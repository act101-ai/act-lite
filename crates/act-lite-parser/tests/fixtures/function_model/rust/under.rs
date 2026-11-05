fn f(a: u8, b: u8, c: u8, d: u8, e: u8) -> u8 {
    let mut y = 0;
    if a > 0 && b > 0 {
        for i in 0..c {
            match i {
                1 => {
                    while y < d {
                        y += 1;
                    }
                }
                _ => y = 3,
            }
        }
    } else if e > 0 {
        y = 4;
    }
    if y > 0 || e > 0 && d > 0 {
        y = 5;
    }
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
    y
}
