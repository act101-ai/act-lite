function f(a: number, b: number, c: number, d: number, e: number): number {
    let y = 0;
    if (a && b) {
        for (let i = 0; i < c; i++) {
            switch (i) {
                case 1:
                    try {
                        y = d;
                    } catch (x) {
                        y = 2;
                    }
                    break;
                default:
                    y = 3;
            }
        }
    } else if (e) {
        y = 4;
    }
    y = e ? 1 : 2;
    if (y > 0 || e) {
        y = 5;
    }
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    y = y + 1;
    return y;
}
