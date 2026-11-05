function f(a, b, c, d, e)
    y = 0
    if a > 0 && b > 0
        for i in 1:c
            while y < d
                try
                    y += 1
                catch
                    y = 0
                end
            end
        end
    elseif e > 0 || d > 0
        y = 4
    end
    w = y > 9 && d > 0 ? 9 : y
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y += 1
    y + w
end
