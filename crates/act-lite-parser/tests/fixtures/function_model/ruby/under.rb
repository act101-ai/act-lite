def f(a, b, c, d, e)
  y = 0
  if a && b
    while y < c
      case y
      when 1
        begin
          y = d
        rescue StandardError
          y = 2
        end
      else
        y = 3
      end
    end
  elsif e
    y = 4
  end
  y = e ? 1 : 2
  if y > 0 || e
    y = 5
  end
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
end
