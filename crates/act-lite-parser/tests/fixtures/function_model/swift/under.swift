func f(a: Int, b: Int, c: Int, d: Int, e: Int) -> Int {
    var y = 0
    if a > 0 && b > 0 {
        for i in 0..<c {
            switch i {
            case 1:
                do {
                    y = try g(d)
                } catch {
                    y = 2
                }
            default:
                y = 3
            }
        }
    } else if e > 0 {
        y = 4
    }
    y = e > 0 ? 1 : 2
    if y > 0 || e > 0 {
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
