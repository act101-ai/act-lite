function f(a, b, c, d, e, g)
    y = 0
    if a > 0 && b > 0
        for i in 1:c
            while y < d
                try
                    if g > 0 || e > 0
                        y += 1
                    end
                catch err
                    y = 0
                end
            end
        end
    elseif e > 0
        y = 4
    else
        y = 5
    end
    z = map(v -> v + 1, y)
    w = y > 9 && z > 0 ? 9 : y
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
    y + z + w
end
g(x)::Int = x + 1
h(x::T) where {T} = x
macro twice(ex)
    ex
end
k(v) = map(v) do u
    u * 2
end
