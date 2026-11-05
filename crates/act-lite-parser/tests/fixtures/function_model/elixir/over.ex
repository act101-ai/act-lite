defmodule M do
  def f(a, b, c, d, e, g) do
    y = 0
    if a > 0 && b > 0 do
      for i <- c do
        case i do
          1 ->
            try do
              if g do
                d
              end
            rescue
              _ -> 2
            end
          _ ->
            3
        end
      end
    end
    if e > 0 || (y > 0 and d) do
      5
    end
    unless e do
      4
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
    y
  end

  defp helper(x), do: x
end
