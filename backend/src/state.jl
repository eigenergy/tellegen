# Case registry. Entries are read-only after boot except `prob`, whose demand
# vector, JuMP model, and sensitivity cache mutate under `lock` as
# perturbation requests come in. `base_d` keeps the boot demand so every
# request can re-establish its own absolute state (base + deltas) instead of
# accumulating mutations.

struct CaseEntry
    id::String
    name::String
    case::ParsedCase
    net::DCNetwork
    prob::DCOPFProblem
    base_d::Vector{Float64}
    coords::Dict{Int,NTuple{2,Float64}}
    lock::ReentrantLock
    network_json::String
    solution_json::String
end

const CASES = Dict{String,CaseEntry}()

# Operator-staged TAMU distributions (scripts/stage-data.sh); never vendored.
const DATA_DIR = normpath(get(ENV, "TELLEGEN_DATA", joinpath(@__DIR__, "..", "..", "data")))

# The demo cases are TAMU ACTIVSg synthetic grids, served at the real
# substation coordinates carried in their aux exports. Quadratic generator
# costs make dLMP/dd nonzero across the interior of the feasible region;
# linear cost cases (most IEEE test systems) have piecewise constant LMPs
# whose gradient is zero almost everywhere.
const CASE_SPECS = (
    (id="case200", name="ACTIVSg200 (Illinois)",
        casefile="ACTIVSg200/case_ACTIVSg200.m", auxfile="ACTIVSg200/ACTIVSg200.aux"),
    (id="case500", name="ACTIVSg500 (South Carolina)",
        casefile="ACTIVSg500/case_ACTIVSg500.m", auxfile="ACTIVSg500/ACTIVSg500.aux"),
    (id="case2000", name="ACTIVSg2000 (Texas)",
        casefile="ACTIVSg2000/case_ACTIVSg2000.m", auxfile="ACTIVSg2000/ACTIVSg2000.aux"),
)

# Without staged data the server still boots: pglib variants of the small
# cases, placed by the spectral layout in layout.jl. A dev convenience; the
# deploy stages real data.
const FALLBACK_SPECS = (
    (id="case200", name="ACTIVSg200 (Illinois)", file="pglib_opf_case200_activ.m",
        bbox=(-91.4, 37.1, -87.6, 42.4)),
    (id="case500", name="ACTIVSg500 (South Carolina)", file="pglib_opf_case500_goc.m",
        bbox=(-82.9, 33.3, -79.9, 35.0)),
)

_staged(spec) =
    isfile(joinpath(DATA_DIR, spec.casefile)) && isfile(joinpath(DATA_DIR, spec.auxfile))

function load_cases!()
    empty!(CASES)  # idempotent: a reload rebuilds rather than accumulating stale entries
    specs = filter(_staged, CASE_SPECS)
    isempty(specs) &&
        @warn "no TAMU case data under $DATA_DIR; serving pglib fallbacks with synthetic layout (see scripts/stage-data.sh)"
    for spec in specs
        # One bad distribution should not take down the cases that load.
        try
            CASES[spec.id] = build_entry(spec)
            @info "case loaded" spec.id
        catch err
            @error "case failed to load" spec.id err
        end
    end
    if isempty(CASES)
        for spec in FALLBACK_SPECS
            # Same guard for the fallbacks: one bad pglib case should not
            # block the other from booting.
            try
                CASES[spec.id] = build_entry(spec)
                @info "case loaded (fallback)" spec.id
            catch err
                @error "fallback case failed to load" spec.id err
            end
        end
    end
end

function build_entry(spec)
    if haskey(spec, :auxfile)
        case = parse_file(joinpath(DATA_DIR, spec.casefile))
        coords = real_coords(joinpath(DATA_DIR, spec.auxfile))
        unmapped = [b.bus_i for b in case.bus if !haskey(coords, b.bus_i)]
        isempty(unmapped) || error(
            "$(spec.id): aux carries no coordinates for buses $(unmapped[1:min(end, 5)])")
        synthetic = false
    else
        case = parse_file(spec.file; library=:pglib)
        coords = synthetic_layout(case; bbox=spec.bbox)
        synthetic = true
    end
    net = DCNetwork(case)
    prob = DCOPFProblem(net)
    sol = solve!(prob)
    # Warm the dLMP/dd cache so request handlers hit a populated cache.
    calc_sensitivity(prob, :lmp, :d)
    network_json = JSON3.write(network_payload(spec, case, coords; synthetic))
    solution_json = JSON3.write(solution_payload(case, net, sol))
    return CaseEntry(spec.id, spec.name, case, net, prob, copy(prob.d), coords,
        ReentrantLock(), network_json, solution_json)
end

"""
    establish_demand!(e, deltas) -> DCOPFSolution

Set the problem demand to base + deltas (MW keyed by bus id) and return the
solution at that point, re-solving only when the demand actually changes.
Caller must hold `e.lock`.
"""
function establish_demand!(e::CaseEntry, deltas)
    target = target_demand(e, deltas)
    if target != e.prob.d || e.prob.cache.solution === nothing
        update_demand!(e.prob, target)
        return solve!(e.prob)
    end
    return e.prob.cache.solution
end

function target_demand(e::CaseEntry, deltas)
    target = copy(e.base_d)
    base = e.case.baseMVA
    for (bus, mw) in deltas
        i = get(e.net.id_map.bus_to_idx, bus, nothing)
        isnothing(i) && continue
        target[i] += mw / base
    end
    return target
end

function network_payload(spec, case::ParsedCase, coords; synthetic::Bool)
    base = case.baseMVA
    demand_mw = Dict{Int,Float64}()
    for l in case.load
        l.status == 1 || continue
        demand_mw[l.load_bus] = get(demand_mw, l.load_bus, 0.0) + l.pd * base
    end
    gen_mw = Dict{Int,Float64}()
    for g in case.gen
        g.gen_status == 1 || continue
        gen_mw[g.gen_bus] = get(gen_mw, g.gen_bus, 0.0) + g.pmax * base
    end
    buses = [(id=b.bus_i,
              lon=coords[b.bus_i][1],
              lat=coords[b.bus_i][2],
              demand_mw=get(demand_mw, b.bus_i, 0.0),
              gen_mw=get(gen_mw, b.bus_i, 0.0))
             for b in case.bus]
    branches = [(id=br.index,
                 from=br.f_bus,
                 to=br.t_bus,
                 rate_mw=br.rate_a * base,
                 status=br.br_status,
                 path=[collect(coords[br.f_bus]), collect(coords[br.t_bus])])
                for br in case.branch]
    return (id=spec.id, name=spec.name, base_mva=base, synthetic_coords=synthetic,
        buses=buses, branches=branches)
end

function solution_payload(case::ParsedCase, net::DCNetwork, sol)
    base = case.baseMVA
    lmp = calc_lmp(sol, net)
    bus_ids = net.id_map.bus_ids
    branch_ids = net.id_map.branch_ids
    gen_ids = net.id_map.gen_ids
    # Duals are taken with respect to per unit power; /base puts LMPs per MWh.
    return (objective=sol.objective,
        lmp=[(bus=bus_ids[i], usd_per_mwh=lmp[i] / base) for i in 1:net.n],
        flows=[(branch=branch_ids[e],
                mw=sol.f[e] * base,
                loading=net.fmax[e] > 0 ? abs(sol.f[e]) / net.fmax[e] : 0.0)
               for e in 1:net.m],
        dispatch=[(gen=gen_ids[g], mw=sol.pg[g] * base) for g in 1:net.k])
end

# Caller must hold `e.lock`: calc_sensitivity reads/writes the cache.
function sensitivity_payload(e::CaseEntry, bus_id::Int)
    S = calc_sensitivity(e.prob, :lmp, :d)
    base = e.case.baseMVA
    col = S.id_to_col[bus_id]
    bus_ids = e.net.id_map.bus_ids
    # Both sides of dLMP/dd are per unit; /base^2 converts to ($/MWh)/MW.
    values = [(bus=bus_ids[i], value=S[i, col] / base^2) for i in 1:e.net.n]
    return (case=e.id, operand="lmp", parameter="d", bus=bus_id,
        units="(\$/MWh)/MW", values=values)
end
