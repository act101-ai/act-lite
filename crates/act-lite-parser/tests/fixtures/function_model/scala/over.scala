object O {
  def f(a: Int, b: Int, c: Int, d: Int, e: Int, g: Int): Int = {
    var y = 0
    if (a > 0 && b > 0) {
      for (i <- 0 to c) {
        i match {
          case 1 =>
            try {
              if (g > 0) {
                y = d
              }
            } catch {
              case x: Exception => y = 2
            }
          case _ =>
            y = 3
        }
      }
    } else if (e > 0) {
      y = 4
    }
    if (y > 0 || e > 0 && d > 0) {
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
    y
  }

  def m(x: Int)(z: Int): Int = x + z
}
