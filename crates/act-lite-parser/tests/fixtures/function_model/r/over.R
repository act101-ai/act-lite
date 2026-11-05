f <- function(a, b, c, d, e, g) {
  y <- 0
  if (a > 0 && b > 0) {
    for (i in seq_len(c)) {
      while (y < d) {
        repeat {
          if (g > 0 || e > 0) {
            y <- y + 1
          }
          break
        }
      }
    }
  } else if (e > 0) {
    y <- 4
  } else {
    y <- 5
  }
  z <- sapply(y, function(y) y + 1)
  if (z > 9 && y > 9) { z <- 9 }
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y <- y + 1
  y + z
}
m = function(k) {
  k + 1
}
