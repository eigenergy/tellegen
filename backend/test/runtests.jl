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

@testset "synthetic layout" begin
    case = parse_file("pglib_opf_case500_goc.m"; library=:pglib)
    bbox = (-82.9, 33.3, -79.9, 35.0)
    coords = Tellegen.synthetic_layout(case; bbox)
    @test length(coords) == length(case.bus)
    pts = collect(values(coords))
    @test all(bbox[1] <= p[1] <= bbox[3] && bbox[2] <= p[2] <= bbox[4] for p in pts)
    # No stacked buses: the force pass must separate every pair.
    mind = minimum(
        hypot(pts[i][1] - pts[j][1], pts[i][2] - pts[j][2]) for
        i in eachindex(pts), j in eachindex(pts) if i < j
    )
    @test mind > 1e-5
    # The layout should fill its territory, not collapse into a filament:
    # demand a meaningful spread along both axes.
    for axis in (1, 2)
        vals = sort([p[axis] for p in pts])
        iqr = vals[ceil(Int, 0.75 * end)] - vals[ceil(Int, 0.25 * end)]
        @test iqr > 0.2 * (bbox[axis + 2] - bbox[axis])
    end
end

@testset "real coordinates from aux" begin
    spec = first(Tellegen.CASE_SPECS)
    if Tellegen._staged(spec)
        coords = Tellegen.real_coords(joinpath(Tellegen.DATA_DIR, spec.auxfile))
        @test length(coords) == 200
        # Inside the Illinois footprint the case was built on.
        @test all(-92 < c[1] < -87 && 37 < c[2] < 43 for c in values(coords))
        # Stack spreading leaves no two buses at the same point.
        pts = collect(values(coords))
        @test length(unique(pts)) == length(pts)
        @test minimum(
            hypot(pts[i][1] - pts[j][1], pts[i][2] - pts[j][2]) for
            i in eachindex(pts), j in eachindex(pts) if i < j
        ) > 1e-5
    else
        @info "TAMU data not staged; skipping real coordinate tests"
    end
end

@testset "case registry payloads" begin
    # The TAMU spec when its data is staged, the pglib fallback otherwise,
    # so the testset runs on machines without the distributions.
    spec = Tellegen._staged(first(Tellegen.CASE_SPECS)) ? first(Tellegen.CASE_SPECS) :
        first(Tellegen.FALLBACK_SPECS)
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
