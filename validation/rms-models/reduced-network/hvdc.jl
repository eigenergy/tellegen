# Reference for HVDC links in AC emulation: two classical machines joined
# by a weak tie, with a link between the tie's ends, through a load step,
# and the inter-area mode of the linearized system. Writes `hvdc.json`.
#
#   julia --project=validation/rms-models validation/rms-models/reduced-network/hvdc.jl

include(joinpath(@__DIR__, "common.jl"))

# Two areas: the classical machines at 1 and 2 (500 MVA, X'd 0.3 pu, H 4 s,
# no damping) feed the loads at 3 and 4 (250 MW each, constant impedance)
# over lines of 0.02 pu; the tie 3-4 has 0.3 pu. Machine 1 is the slack and
# its speed the frame.
const TWO_AREAS = Case(
    [Bus(1, 3, 1.0), Bus(2, 2, 1.0), Bus(3, 1, 1.0), Bus(4, 1, 1.0)],
    [Gen(1, 0.0, 1.0, 500.0, 0.3), Gen(2, 200.0, 1.0, 500.0, 0.3)],
    [Load(3, 250.0, 0.0), Load(4, 250.0, 0.0)],
    [Line(1, 3, "1", 0.02), Line(2, 4, "1", 0.02), Line(3, 4, "1", 0.3)],
    Dict(1 => (4.0, 0.0), 2 => (4.0, 0.0)),
)

const T_END = 5.0
const DT_OUT = 0.02

# The load at 4 up by a fifth, off the 20 ms sample grid.
const EVENTS = Dict("step" => [(0.105, :scale, 4, 1.2)])

# The runs: name, event, links (from, to, K MW/degree, limit MW, 0 = none).
const RUNS = [
    ("base", "step", Tuple{Int, Int, Float64, Float64}[]),
    ("link", "step", [(3, 4, 50.0, 0.0)]),
    ("link_limited", "step", [(3, 4, 50.0, 20.0)]),
]

const CHANNELS = [
    "omega:1", "omega:2", "delta:1", "delta:2", "va:1", "va:3", "va:4", "pe:1", "pe:2",
    "p_from:3-4",
]

sw(links) = Switches(load_model = :impedance, links = links)

"""The oscillatory mode of the eigenvalues `λ` (the pair with the largest imaginary part)."""
function inter_area(λ)
    k = argmax(imag.(λ))
    @assert count(x -> abs(imag(x)) > 1e-6, λ) == 2 "one oscillatory pair: $λ"
    return λ[k]
end

"""
The two areas in PowerSimulationsDynamics.jl 0.16.2 (unmodified), the loads
as constant impedance, with `extra` lines besides the case's.
"""
function psid_two_areas(extra = Line[])
    c = Case(TWO_AREAS.buses, TWO_AREAS.gens, TWO_AREAS.loads, vcat(TWO_AREAS.lines, extra), TWO_AREAS.machines)
    sys = psid_system(c)
    for l in PSY.get_components(PSY.StandardLoad, sys)
        transform_load_to_constant_impedance(l)
    end
    return sys
end

"""
The model here without a link against PowerSimulationsDynamics.jl through the
load step: the largest difference of each channel (angles against machine 1
and bus 1).
"""
function psid_check(chs)
    sys = psid_two_areas()
    perts = psid_perturbations(sys, EVENTS["step"]; load_part = :impedance)
    sim, res = psid_run(sys, T_END, DT_OUT, perts)
    grid = collect(0:DT_OUT:T_END+1e-9)
    on_grid((t, x)) = psid_on_grid(t, x, grid)
    g1, g2 = "generator-1-1", "generator-2-1"
    d1 = on_grid(get_state_series(res, (g1, :δ)))
    v1 = on_grid(get_voltage_angle_series(res, 1))
    theirs = Dict(
        "omega:1" => on_grid(get_state_series(res, (g1, :ω))),
        "omega:2" => on_grid(get_state_series(res, (g2, :ω))),
        "delta:2" => on_grid(get_state_series(res, (g2, :δ))) .- d1,
        "va:3" => on_grid(get_voltage_angle_series(res, 3)) .- v1,
        "va:4" => on_grid(get_voltage_angle_series(res, 4)) .- v1,
        "pe:1" => on_grid(get_activepower_series(res, g1)),
        "pe:2" => on_grid(get_activepower_series(res, g2)),
    )
    ours = Dict(
        "omega:1" => [c["omega:1"] for c in chs],
        "omega:2" => [c["omega:2"] for c in chs],
        "delta:2" => [c["delta:2"] - c["delta:1"] for c in chs],
        "va:3" => [c["va:3"] - c["va:1"] for c in chs],
        "va:4" => [c["va:4"] - c["va:1"] for c in chs],
        "pe:1" => [c["pe:1"] for c in chs],
        "pe:2" => [c["pe:2"] for c in chs],
    )
    worst = Dict(k => maximum(abs.(theirs[k] .- ours[k])) for k in keys(theirs))
    println("  PowerSimulationsDynamics.jl against the model, without a link: ",
        join(["$k $(@sprintf("%.1e", v))" for (k, v) in sort(collect(worst))], ", "))
    return worst
end

function main()
    runs = []
    check = nothing
    for (name, event, links) in RUNS
        m, u0 = build(TWO_AREAS, sw(links))
        ts, us, chs = simulate(m, u0, T_END, DT_OUT, EVENTS[event])
        keys = isempty(links) ? CHANNELS : vcat(CHANNELS, "link:3-4")
        if !isempty(links)
            p = [c["link:3-4"] for c in chs]
            println(@sprintf("%-12s link power %.4f..%.4f pu", name, minimum(p), maximum(p)))
        end
        if name == "base"
            check = psid_check(chs)
        end
        push!(runs, Dict(
            "name" => name,
            "event" => event,
            "switches" => isempty(links) ? Dict() : Dict("hvdc_emulation" => [collect(Any, l) for l in links]),
            "time" => ts,
            "channels" => export_channels(chs, keys),
        ))
    end

    # The inter-area mode: the model linearized at the start with a link of
    # K = 0, 10, 50 MW/degree; PowerSimulationsDynamics.jl's without a link,
    # and with an AC line 3-4 in parallel to the tie in place of the link, of
    # reactance 1/K (K in pu power per radian) and of the reactance whose
    # synchronizing power V3 V4 cos(θ3 − θ4) / x at the start equals K.
    modes = []
    V = power_flow(TWO_AREAS)
    sync = abs(V[3]) * abs(V[4]) * cos(angle(V[3]) - angle(V[4]))
    for k in [0.0, 10.0, 50.0]
        links = k == 0 ? Tuple{Int, Int, Float64, Float64}[] : [(3, 4, k, 0.0)]
        m, u0 = build(TWO_AREAS, sw(links))
        λ = inter_area(eigenvalues(m, u0))
        mode = Dict{String, Any}("k_mw_per_degree" => k, "eigenvalue" => [real(λ), imag(λ)])
        if k == 0
            sim = Simulation(MassMatrixModel, psid_two_areas(), mktempdir(), (0.0, 1.0))
            μ = inter_area(small_signal_analysis(sim).eigenvalues)
            mode["psid"] = [real(μ), imag(μ)]
        else
            kpu = k * 180 / π / SBASE
            for (tag, x) in [("psid_ac_line_inverse_k", 1 / kpu), ("psid_ac_line_matched", sync / kpu)]
                sim = Simulation(MassMatrixModel, psid_two_areas([Line(3, 4, "2", x)]), mktempdir(), (0.0, 1.0))
                μ = inter_area(small_signal_analysis(sim).eigenvalues)
                mode[tag] = [real(μ), imag(μ), x]
            end
        end
        println("  K = $k MW/degree: ", join(["$t $(mode[t])" for t in sort(collect(keys(mode)))], ", "))
        push!(modes, mode)
    end
    write_json("hvdc.json", Dict(
        "case" => "two_areas",
        "psid_check" => check,
        "runs" => runs,
        "inter_area_mode" => modes,
    ))
end

if abspath(PROGRAM_FILE) == @__FILE__
    main()
end
