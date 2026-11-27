# Closed-loop single-machine references for the controllers of
# controllers.jl, run in PowerSimulationsDynamics.jl 0.16.2.
#
#   julia --project=validation/rms-models validation/rms-models/reduced-controllers/run.jl
#
# writes crates/tellegen/tests/data/rms_reduced_controllers/closed_loop.json.

using Logging
using JSON3
using OrdinaryDiffEq
include(joinpath(@__DIR__, "controllers.jl"))

const ROOT = normpath(joinpath(@__DIR__, "..", "..", ".."))
const DATA = joinpath(ROOT, "crates", "tellegen", "tests", "data", "rms_reduced_controllers")

# Solver settings of every run.
const TOL = 1e-9
const SOLVER = "Rodas5P"

"""
The single-machine system of `case` (raw and dyr in DATA) with the unit's
exciter and/or governor replaced, and a load at the unit's bus that carries
nothing until a fault: a constant impedance whose reactive power at the
power flow voltage is set by the fault events.
"""
function build(case; avr = nothing, tg = nothing)
    sys = with_logger(NullLogger()) do
        System(joinpath(DATA, case * ".raw"), joinpath(DATA, case * ".dyr"); runchecks = false)
    end
    dg = only(get_components(DynamicGenerator, sys))
    gen = get_component(ThermalStandard, sys, get_name(dg))
    unit = DynamicGenerator(;
        name = get_name(dg),
        ω_ref = 1.0,
        machine = get_machine(dg),
        shaft = get_shaft(dg),
        avr = avr === nothing ? get_avr(dg) : avr,
        prime_mover = tg === nothing ? get_prime_mover(dg) : tg,
        pss = get_pss(dg),
        base_power = get_base_power(dg),
    )
    remove_component!(sys, dg)
    add_component!(sys, unit, gen)
    bus = get_bus(gen)
    fault = StandardLoad(;
        name = "fault",
        available = true,
        bus = bus,
        base_power = get_base_power(sys),
    )
    add_component!(sys, fault)
    return sys, unit, gen, fault
end

"""
The events of a run as PowerSimulationsDynamics.jl perturbations. Every event
is `(time, kind, value)`: `"trip"` opens circuit 2, `"fault"` puts a reactance
`value` (system pu) from the unit's bus to ground, `"clear"` removes it,
`"v_ref"` / `"p_ref"` set the reference to `value`.
"""
function perturbations(sys, unit, fault, events)
    out = PSID.Perturbation[]
    V0 = get_magnitude(get_bus(fault))
    x_fault = 0.0
    for (t, kind, value) in events
        if kind == "trip"
            line = only(l for l in get_components(Line, sys) if endswith(get_name(l), "_2"))
            push!(out, BranchTrip(t, Line, get_name(line)))
        elseif kind == "fault"
            x_fault = value
            # LoadChange adds the difference to the load's own (zero) value.
            push!(out, LoadChange(t, fault, :Q_ref_impedance, V0^2 / value))
        elseif kind == "clear"
            push!(out, LoadChange(t, fault, :Q_ref_impedance, -V0^2 / x_fault))
        elseif kind == "v_ref"
            push!(out, ControlReferenceChange(t, unit, :V_ref, value))
        elseif kind == "p_ref"
            push!(out, ControlReferenceChange(t, unit, :P_ref, value))
        else
            error("unknown event $kind")
        end
    end
    return out
end

"Runs `f` with only errors logged."
quiet(f) = with_logger(f, SimpleLogger(stderr, Logging.Error))

"The reference in effect at `t` (the initial one before any event of `kind`)."
function reference_at(events, kind, initial, t)
    v = initial
    for (te, k, value) in events
        if k == kind && te <= t
            v = value
        end
    end
    return v
end

"""
Runs `events` over `t_end` and returns the channels every `sample` seconds: the
network and machine quantities, and the controller's own signals.
"""
function run_case(case, events, t_end, sample; avr = nothing, tg = nothing)
    # The initial references, from a run without events (a step is given
    # relative to them).
    sys, unit, gen, fault = build(case; avr = avr, tg = tg)
    sim = quiet(() -> Simulation(MassMatrixModel, sys, mktempdir(), (0.0, t_end)))
    sim.status == PSID.BUILT || error("$case: simulation not built")
    w = only(PSID.get_dynamic_injectors(sim.inputs))
    v_ref0 = PSID.get_V_ref(w)
    p_ref0 = PSID.get_P_ref(w)
    events = [
        (t, k, k == "v_ref" ? v_ref0 + v : k == "p_ref" ? p_ref0 + v : v) for (t, k, v) in events
    ]
    sys, unit, gen, fault = build(case; avr = avr, tg = tg)
    a = PSY.get_avr(unit)
    g = PSY.get_prime_mover(unit)
    sim = quiet(
        () -> Simulation(
            MassMatrixModel, sys, mktempdir(), (0.0, t_end), perturbations(sys, unit, fault, events),
        ),
    )
    w = only(PSID.get_dynamic_injectors(sim.inputs))
    stats = @timed execute!(
        sim, Rodas5P(); abstol = TOL, reltol = TOL, dtmax = 0.005, enable_progress_bar = false,
    )
    stats.value == PSID.SIMULATION_FINALIZED || error("$case: simulation failed")
    res = PSID.read_results(sim)
    sol = res.solution
    bus_ix = PSID.get_bus_lookup(res)
    n_bus = length(bus_ix)
    b = bus_ix[102]
    times = collect(0.0:sample:(t_end + 1e-9))
    mbase = get_base_power(unit)
    sbase = get_base_power(sys)
    ch = Dict{String, Vector{Float64}}()
    push_ch!(k, v) = push!(get!(ch, k, Float64[]), v)
    names = PSY.get_states(unit)
    ix = PSID.get_ix_range(w)
    for t in times
        u = sol(t)
        x = u[ix]
        # The device's inputs at `t`: its bus voltage and the references in
        # effect; the field current and torques from the device's own models.
        inner = zeros(PSID.GEN_INNER_VARS_SIZE)
        inner[Int(PSID.VR_gen_var)] = u[b]
        inner[Int(PSID.VI_gen_var)] = u[b + n_bus]
        PSID.set_V_ref(w, reference_at(events, "v_ref", v_ref0, t))
        PSID.set_P_ref(w, reference_at(events, "p_ref", p_ref0, t))
        PSID._update_inner_vars!(x, similar(x), 1.0, inner, w)
        scratch = similar(x)
        PSID.mdl_tg_ode!(x, scratch, inner, 1.0, w, nothing, t)
        PSID.mdl_avr_ode!(x, scratch, inner, w, nothing, t)
        V_R, V_I, I_R, I_I, P, Q = terminal(x, inner, w)
        Vt = hypot(V_R, V_I)
        push_ch!("vm:102", Vt)
        push_ch!("omega:102", x[findfirst(==(:ω), names)])
        push_ch!("delta:102", x[findfirst(==(:δ), names)])
        push_ch!("pe:102", P * mbase / sbase)
        push_ch!("qe:102", Q * mbase / sbase)
        push_ch!("efd:102", inner[Int(PSID.Vf_var)])
        push_ch!("tm:102", inner[Int(PSID.τm_var)])
        if a isa RefESAC6A
            Ifd = inner[Int(PSID.Xad_Ifd_var)]
            xa = x[[findfirst(==(s), names) for s in ESAC6A_STATES]]
            _, sig = esac6a_eval(a, xa, PSID.get_V_ref(w), 0.0, Vt, Ifd, P, Q)
            push_ch!("sig:exciter.ifd:102", Ifd)
            for (k, v) in pairs(sig)
                k == :Efd && continue
                # A limiter's signals only where the unit has it.
                a.oel === nothing && (k == :VOEL || startswith(String(k), "oel")) && continue
                a.uel === nothing && (k == :VUEL || startswith(String(k), "uel")) && continue
                push_ch!("sig:exciter.$(k):102", v)
            end
        end
    end
    return Dict(
        "time" => times,
        "channels" => ch,
        "v_ref0" => v_ref0,
        "p_ref0" => p_ref0,
        "events" => [Dict("time" => t, "kind" => k, "value" => v) for (t, k, v) in events],
        "seconds" => stats.time,
    )
end

# ---------------------------------------------------------------------------
# The runs.

"The per-circuit line reactance of a case (system pu), read from its raw file."
function x_line(case)
    for l in eachline(joinpath(DATA, case * ".raw"))
        f = split(l, ',')
        length(f) > 5 && strip(f[1]) == "101" && strip(f[2]) == "102" && return parse(Float64, f[5])
    end
    error("$case: no line")
end

const BASE = "smib_genrou_sexs_tgov1"
const GAST_CASE = "smib_genrou_gast_sexs"

# ESAC6A: the set of tellegen's model library (src/rms/library.rs: the
# regulator of PowerSystems.jl 5.12.5's test ESST1A, the transducer and AC
# exciter of PowerSimulationsDynamics.jl's IEEE 14-bus ESAC1A).
const ESAC6A_TYPICAL =
    [0.001, 200.0, 0.1, 0.0, 13.25, 2.5, 9.5, -9.5, 7.32, -7.32, 0.8, 99.0, 0.0, 99.0, 0.0, 0.0,
     0.2, 0.38, 1.0, 3.14, 0.03, 4.18, 0.1]
# ESAC6A with its limits narrowed around the operating point (at rest
# VA = VR = VFE = 2.92, Vt = 1.014): VAMAX/VAMIN 4/2, VRMAX/VRMIN 3.6/2.85,
# and the exciter field current limiter on (VFELIM 2.93, KH 2, VHMAX 0.3,
# TH 0.2, TJ 0.1).
const ESAC6A_LIMITED = let p = copy(ESAC6A_TYPICAL)
    p[7:10] .= (4.0, 2.0, 3.6, 2.85)
    p[12:16] .= (2.93, 2.0, 0.3, 0.2, 0.1)
    p
end
# The same with VFELIM 2.95: at rest KH (VFE - VFELIM) = -0.051 lies further
# below the limiter's zero floor.
const ESAC6A_LIMITED_VFELIM = let p = copy(ESAC6A_LIMITED)
    p[12] = 2.95
    p
end
# OELIT: IFDRAT 2 (rest Ifd 1.96), pickup 1.02, continuous 1.05, instantaneous
# 1.15 (2.3 pu), C 1 s, TCOOL 10 s, KP 0.2, KI 0.2, VOELMIN -0.3.
const OELIT = [2.0, 1.02, 1.05, 1.15, 1.0, 10.0, 0.2, 0.2, -0.3]
# UELPQ: Q0 -0.2, KQP 0.1, KV 2, KP 0.2, KI 0.2, VUELMAX 0.3 (at rest the line
# is at -0.12 against Q = 0.30).
const UELPQ = [-0.2, 0.1, 2.0, 0.2, 0.2, 0.3]
# The unit's own SEXS (TA/TB TB K TE EMIN EMAX) and GAST (R T1 T2 T3 AT KT
# VMAX VMIN DTURB).
const SEXS = [0.1, 10.0, 100.0, 0.1, 0.0, 3.0]
const GAST = [0.08, 0.3, 0.3, 3.0, 1.0, 2.0, 1.0, 0.0, 0.0]

const RUNS = let xl = x_line(BASE)
    trip = [(1.01, "trip", 0.0)]
    fault = [(1.01, "fault", 0.5 * xl), (1.11, "clear", 0.0)]
    [
        # (id, case, exciter records, switches, events (steps relative), t_end, sample)
        ("esac6a_trip", BASE, Dict("ESAC6A" => ESAC6A_TYPICAL), Dict(), trip, 3.0, 0.02),
        ("esac6a_fault", BASE, Dict("ESAC6A" => ESAC6A_TYPICAL), Dict(), fault, 3.0, 0.02),
        ("esac6a_limits_trip", BASE, Dict("ESAC6A" => ESAC6A_LIMITED), Dict(), trip, 3.0, 0.02),
        ("esac6a_limits_fault", BASE, Dict("ESAC6A" => ESAC6A_LIMITED), Dict(), fault, 3.0, 0.02),
        ("esac6a_vfelim_trip", BASE, Dict("ESAC6A" => ESAC6A_LIMITED_VFELIM), Dict(), trip, 3.0, 0.02),
        (
            "esac6a_oelit_uelpq", BASE,
            Dict("ESAC6A" => ESAC6A_TYPICAL, "OELIT" => OELIT, "UELPQ" => UELPQ), Dict(),
            [(1.01, "v_ref", 0.03), (8.01, "v_ref", -0.06), (13.01, "v_ref", 0.0)], 18.0, 0.05,
        ),
        (
            "sexs_compensator", BASE, Dict(), Dict("vcomp" => [0.01, 0.05, 0.0]),
            [(1.01, "trip", 0.0), (2.01, "v_ref", 0.02)], 4.0, 0.02,
        ),
        (
            "sexs_compensator_lag", BASE, Dict(), Dict("vcomp" => [0.01, 0.05, 0.5]),
            [(1.01, "trip", 0.0), (2.01, "v_ref", 0.02)], 4.0, 0.02,
        ),
        (
            "sexs_uel", BASE, Dict(), Dict("uel" => [0.05, 2.0]),
            [(1.01, "v_ref", -0.08), (3.01, "v_ref", 0.0)], 5.0, 0.02,
        ),
        (
            "gast_rate", GAST_CASE, Dict(), Dict("gast_rate" => 0.05),
            [(1.01, "p_ref", 0.1), (4.01, "p_ref", -0.15)], 11.0, 0.05,
        ),
    ]
end

"The exciter and governor of a run as the components of controllers.jl."
function components(records, switches)
    avr = nothing
    tg = nothing
    if haskey(records, "ESAC6A")
        avr = RefESAC6A(records["ESAC6A"]; oel = get(records, "OELIT", nothing), uel = get(records, "UELPQ", nothing))
    elseif haskey(switches, "vcomp") || haskey(switches, "uel")
        vc = haskey(switches, "vcomp") ? Tuple(switches["vcomp"]) : nothing
        ue = haskey(switches, "uel") ? Tuple(switches["uel"]) : nothing
        avr = RefSEXS(SEXS; vcomp = vc, uel = ue)
    end
    if haskey(switches, "gast_rate")
        tg = RefGAST(GAST; rate = switches["gast_rate"])
    end
    return avr, tg
end

sig7(x) = round(x; sigdigits = 7)

"""
Self-check of the extension: the unit's own SEXS and GAST as written in
controllers.jl (without compensator, limiter or rate limit) against
PowerSimulationsDynamics.jl's SEXS and GasTG, through a line trip and a
reference step. Returns the largest difference of any channel.
"""
function self_check()
    worst = 0.0
    for (case, avr, tg, ev) in [
        (BASE, RefSEXS(SEXS), nothing, [(1.01, "trip", 0.0), (2.01, "v_ref", 0.02)]),
        (GAST_CASE, nothing, RefGAST(GAST), [(1.01, "p_ref", 0.1)]),
    ]
        ours = run_case(case, ev, 4.0, 0.02; avr = avr, tg = tg)["channels"]
        theirs = run_case(case, ev, 4.0, 0.02)["channels"]
        for k in ("vm:102", "omega:102", "delta:102", "pe:102", "qe:102", "efd:102")
            worst = max(worst, maximum(abs.(ours[k] .- theirs[k])))
        end
    end
    return worst
end

function main()
    out = []
    for (id, case, records, switches, events, t_end, sample) in RUNS
        avr, tg = components(records, switches)
        r = run_case(case, events, t_end, sample; avr = avr, tg = tg)
        println(rpad(id, 22), " ", round(r["seconds"]; digits = 2), " s")
        push!(out, Dict(
            "id" => id,
            "case" => case,
            "records" => records,
            "switches" => switches,
            "events" => r["events"],
            "v_ref0" => r["v_ref0"],
            "p_ref0" => r["p_ref0"],
            "t_end" => t_end,
            "time" => sig7.(r["time"]),
            "channels" => Dict(k => sig7.(v) for (k, v) in r["channels"]),
        ))
    end
    check = self_check()
    println("self-check against PowerSimulationsDynamics.jl's SEXS and GasTG: ", check)
    check < 1e-6 || error("the extension does not reproduce PowerSimulationsDynamics.jl")
    about = "PowerSimulationsDynamics.jl 0.16.2 (MassMatrixModel, $SOLVER, reltol = abstol = $TOL, " *
            "dtmax = 5 ms) with the controllers of validation/rms-models/reduced-controllers/" *
            "controllers.jl; see SOURCES.md"
    open(joinpath(DATA, "closed_loop.json"), "w") do io
        JSON3.write(io, Dict("about" => about, "runs" => out))
    end
end

if abspath(PROGRAM_FILE) == @__FILE__
    main()
end
