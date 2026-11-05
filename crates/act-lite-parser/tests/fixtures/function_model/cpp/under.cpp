int f(int a, int b, int c, int d, int e) {
    int y = 0;
    if (a && b) {
        for (int i = 0; i < c; i++) {
            switch (i) {
            case 1:
                try {
                    y = d;
                } catch (int x) {
                    y = x;
                }
                break;
            default:
                y = 3;
                break;
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
    return y;
}
