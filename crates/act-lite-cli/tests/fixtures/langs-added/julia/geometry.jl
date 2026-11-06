module Geometry
using LinearAlgebra
import Base: show
export area
abstract type Shape end
struct Circle <: Shape
    r::Float64
end
mutable struct Box
    w
end
const UNIT = 1.0
function area(c::Circle)
    if c.r > 0
        return pi * c.r^2
    elseif c.r == 0
        return 0.0
    end
    for i in 1:2; end
    while false; end
end
area(b::Box) = b.w^2
macro twice(ex) end
end
