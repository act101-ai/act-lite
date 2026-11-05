fun f(a: Int, b: Int, c: Int, d: Int, e: Int): Int {
    var y = 0
    if (a > 0 && b > 0) {
        for (i in 0..c) {
            when (i) {
                1 -> {
                    try {
                        y = d
                    } catch (x: Exception) {
                        y = 2
                    }
                }
                else -> y = 3
            }
        }
    } else if (e > 0) {
        y = 4
    }
    y = y ?: 1
    if (y > 0 || e > 0) {
        y = 5
    }
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    y = y + 1
    return y
}
