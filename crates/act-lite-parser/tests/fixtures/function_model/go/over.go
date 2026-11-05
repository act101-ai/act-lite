package p

func f(a, b, c, d, e int, g bool) int {
    y := 0
    if a > 0 && b > 0 {
        for i := 0; i < c; i++ {
            switch i {
            case 1:
                for y < d {
                    if g {
                        y++
                    }
                }
            default:
                y = 3
            }
        }
    } else if e > 0 {
        y = 4
    }
    if y > 0 || e > 0 && d > 0 {
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
    y = y + 1
    y = y + 1
    return y
}

type R struct{}

func (r *R) m(x int) int {
    return x
}
