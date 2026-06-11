# Run with: julia --project=backend backend/test/runtests.jl

using Test
using LinearAlgebra
using PowerDiff

include(joinpath(@__DIR__, "..", "src", "app.jl"))

# The sensitivities tellegen serves are exact derivatives of the KKT system,
# not regressions or heuristics. This testset states that claim precisely:
# one dLMP/dd column from PowerDiff matches central finite differences of
# full re-solves to within finite difference truncation error.
@testset "dLMP/dd columns are exact (vs central finite differences)" begin
    case = parse_file("pglib_opf_case200_activ.m"; library=:pglib)
    net = DCNetwork(case)
    prob = DCOPFProblem(net)
    solve!(prob)
    S = calc_sensitivity(prob, :lmp, :d)
    d0 = copy(prob.d)
    h = 1e-2  # 1 MW at 100 MVA base: large enough that finite differences
    # resolve the gradient above solver dual noise, small enough that the
    # active set is stable for the buses below.

    # Test the three buses whose sensitivity columns are largest; near-kink
    # buses (active set changes within +-h) would make the secant compare a
    # different piece of the piecewise smooth LMP surface.
    norms = [norm(S[:, S.id_to_col[net.id_map.bus_ids[i]]]) for i in 1:net.n]
    for i in sortperm(norms; rev=true)[1:3]
        bus = net.id_map.bus_ids[i]
        exact = [S[j, S.id_to_col[bus]] for j in 1:net.n]

        dp = copy(d0)
        dp[i] += h
        update_demand!(prob, dp)
        lmp_p = calc_lmp(solve!(prob), net)

        dm = copy(d0)
        dm[i] -= h
        update_demand!(prob, dm)
        lmp_m = calc_lmp(solve!(prob), net)

        fd = (lmp_p .- lmp_m) ./ (2h)
        rel = norm(fd .- exact) / max(norm(exact), eps())
        @test rel < 1e-3
    end
end

@testset "case registry payloads" begin
    spec = first(Tellegen.CASE_SPECS)
    e = Tellegen.build_entry(spec)

    payload = lock(e.lock) do
        Tellegen.sensitivity_payload(e, first(e.net.id_map.bus_ids))
    end
    @test payload.units == "(\$/MWh)/MW"
    @test length(payload.values) == e.net.n
    @test all(isfinite(v.value) for v in payload.values)

    # Perturbing demand and returning to base reproduces the boot solution.
    sol0 = lock(e.lock) do
        Tellegen.establish_demand!(e, [first(e.net.id_map.bus_ids) => 25.0])
        Tellegen.establish_demand!(e, Pair{Int,Float64}[])
    end
    @test isapprox(sol0.objective, e.prob.cache.solution.objective; rtol=1e-9)

    deltas = Tellegen._parse_deltas("3:12.5,9:-4.0,junk,1:bad")
    @test deltas == [3 => 12.5, 9 => -4.0]
end
