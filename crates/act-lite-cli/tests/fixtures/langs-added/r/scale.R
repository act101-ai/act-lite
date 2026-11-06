library(stats)
scale_by <- function(x, k = 2, ...) {
  if (k > 1) x * k else x
}
center = function(v) v - mean(v)
LIMIT <<- 10
f2 <- \(a) a + 1
for (i in 1:3) print(i)
while (FALSE) next
setClass("Shape", representation(name = "character"))
