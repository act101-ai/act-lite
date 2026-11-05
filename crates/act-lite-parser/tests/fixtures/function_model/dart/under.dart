int f(int a, int b, int c, int d, int e) {
  var y = 0;
  if (a > 0 && b > 0) {
    for (var i = 0; i < c; i++) {
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
  y = y + 1;
  y = y + 1;
  return y;
}
