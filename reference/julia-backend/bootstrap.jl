using Pkg
Pkg.activate(@__DIR__)

include(joinpath(@__DIR__, "src", "app.jl"))

Tellegen.start()
