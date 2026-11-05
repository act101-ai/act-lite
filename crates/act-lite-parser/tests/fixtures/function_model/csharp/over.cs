class A {
    int F(int a, int b, int c, int d, int e, int g) {
        int y = 0;
        if (a > 0 && b > 0) {
            foreach (var i in new[] { c }) {
                switch (i) {
                    case 1:
                        try {
                            if (g > 0) {
                                y = d;
                            }
                        } catch (System.Exception) {
                            y = 2;
                        }
                        break;
                    default:
                        y = 3;
                        break;
                }
            }
        } else if (e > 0) {
            y = 4;
        }
        y = e > 0 ? 1 : 2;
        if (y > 0 || e > 0) {
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
        return y;
    }

    int V {
        get { return 1; }
    }
}
