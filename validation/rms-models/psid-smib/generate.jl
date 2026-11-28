# PowerSimulationsDynamics.jl references for the single-machine cases of the
# extension models (ESST1A, PSS2B): writes
# crates/tellegen/tests/data/rms_smib/psid_reference_ext_models.json.
#
#   julia --project=validation/rms-models validation/rms-models/psid-smib/generate.jl [measured.json]
#
# `measured.json` holds tellegen's differences to a previous version of the
# reference (written by `smib_ext_models_against_psid` with
# RMS_PSID_MEASURED=<file>); without it every tolerance is the floor 1e-6.
# PowerSimulationsDynamics.jl runs as installed: no function is overridden.

using PowerSystems
using PowerSimulationsDynamics
using OrdinaryDiffEq
using JSON3
using Printf
using SparseArrays
using Logging

const PSY = PowerSystems
const PSID = PowerSimulationsDynamics
const PNM = PSID.PNM

const REPO = normpath(joinpath(@__DIR__, "..", "..", ".."))
const WECC = joinpath(REPO, "crates", "tellegen", "tests", "data", "rms_examples", "wecc240")
const OUT = joinpath(
    REPO,
    "crates",
    "tellegen",
    "tests",
    "data",
    "rms_smib",
    "psid_reference_ext_models.json",
)

# ---------------------------------------------------------------------------
# The single-machine case of WECC 240's GENROU + SEXS + TGOV1 unit, written as
# `wecc240_smib_cases` in crates/tellegen/tests/support/smib.rs writes it.
# ---------------------------------------------------------------------------

const SMIB_TAIL = """ 0 /End of Branch data, Begin Transformer data
 0 /End of Transformer data, Begin Area interchange data
 0 /End of Area interchange data, Begin Two-terminal dc line data
 0 /End of Two-terminal dc line data, Begin VSC dc line data
 0 /End of VSC dc line data, Begin Impedance correction table data
 0 /End of Impedance correction table data, Begin Multi-terminal dc line data
 0 /End of Multi-terminal dc line data, Begin Multi-section line data
 0 /End of Multi-section line data, Begin Zone data
 0 /End of Zone data, Begin Inter-area transfer data
 0 /End of Inter-area transfer data, Begin Owner data
 0 /End of Owner data, Begin FACTS device data
 0 /End of FACTS device data, Begin Switched shunt data
 0 /End of Switched shunt data, Begin GNE device data
 0 /End of GNE device data
Q
"""

f103(x) = @sprintf("%10.3f", x)
sci5(x) = @sprintf("%.5E", x)
unquote(s) = strip(strip(strip(s), '\''))

"""The base case: raw text, the unit's DYR records (model => parameter
tokens), its inertia H and the per-circuit line reactance."""
function base_case()
    raw = read(joinpath(WECC, "case.raw"), String)
    lines = split(raw, '\n')
    start = findfirst(l -> occursin("Begin Generator data", l), lines) + 1
    stop = findfirst(l -> occursin("End of Generator data", l), lines) - 1
    gens = Dict{Tuple{String, String}, Vector{String}}()
    for l in lines[start:stop]
        f = String.(strip.(split(l, ',')))
        gens[(f[1], String(unquote(f[2])))] = f
    end
    dyr = read(joinpath(WECC, "case.dyr"), String)
    order = Tuple{String, String}[]
    records = Dict{Tuple{String, String}, Vector{Vector{String}}}()
    for rec in split(dyr, '/')
        tok = String.(split(replace(strip(rec), ',' => ' ')))
        isempty(tok) && continue
        key = (tok[1], String(unquote(tok[3])))
        haskey(records, key) || push!(order, key)
        push!(get!(records, key, Vector{String}[]), tok)
    end
    unit = nothing
    for key in order
        haskey(gens, key) || continue
        gens[key][15] == "1" || continue
        models = sort([String(unquote(t[2])) for t in records[key]])
        if models == ["GENROU", "SEXS", "TGOV1"]
            unit = key
            break
        end
    end
    g = gens[unit]
    num(i) = parse(Float64, g[i + 1])
    pg, mbase, zr, zx, vs = num(2), num(8), num(9), num(10), num(6)
    x_line = 0.1 * 100.0 / mbase
    r = IOBuffer()
    print(r, "0,   100.00,  32, 0, 1, 60.00     / SMIB of a WECC 240 unit\n\n\n")
    print(r, "   101,'INF BUS', 230.0000,3,   1,   1,   1,1.00000,   0.0000\n")
    print(r, "   102,'UNIT',    230.0000,2,   1,   1,   1,", @sprintf("%.5f", vs), ",   0.0000\n")
    print(r, " 0 /End of Bus data, Begin Load data\n 0 /End of Load data, Begin Fixed shunt data\n")
    print(r, " 0 /End of Fixed shunt data, Begin Generator data\n")
    print(
        r,
        "   101,'1 ', ",
        f103(-pg),
        ",     0.000, 99999.000, -99999.000,1.00000,     0,    100.000, ",
        "0.00000E+0, 1.00000E-5, 0.00000E+0, 0.00000E+0,1.00000,1,  100.0, 99999.000, -99999.000,   1,1.0000\n",
    )
    print(
        r,
        "   102,'1 ', ",
        f103(pg),
        ",     0.000, ",
        f103(num(4)),
        ", ",
        f103(num(5)),
        ",",
        @sprintf("%.5f", vs),
        ",     0, ",
        f103(mbase),
        ", ",
        sci5(zr),
        ", ",
        sci5(zx),
        ", 0.00000E+0, 0.00000E+0,1.00000,1,  100.0, ",
        f103(num(16)),
        ", ",
        f103(num(17)),
        ",   1,1.0000\n",
    )
    print(r, " 0 /End of Generator data, Begin Branch data\n")
    for ckt in ("1", "2")
        print(
            r,
            "   101,    102,'$ckt ', 0.00000E+0, ",
            sci5(x_line),
            ",   0.00000,  9999.00,  9999.00,  9999.00,  ",
            "0.00000,  0.00000,  0.00000,  0.00000,1,1,   0.00,   1,1.0000\n",
        )
    end
    print(r, SMIB_TAIL)
    params = Dict(String(unquote(t[2])) => t[4:end] for t in records[unit])
    h = parse(Float64, params["GENROU"][5])
    return (raw = String(take!(r)), params = params, h = h, x_line = x_line)
end

# ---------------------------------------------------------------------------
# The variants (see SOURCES.md for where the sets come from).
# ---------------------------------------------------------------------------

# tellegen's library ESST1A (src/rms/library.rs: the ESST1A of
# PowerSystems.jl 5.12.5's test/test_dynamic_generator.jl), ICONs UEL VOS first.
const ESST1A_LIBRARY =
    "1 1 0.0 99.0 -99.0 2.5 13.25 0.0 0.0 200.0 0.1 9.5 -9.5 9.5 -9.5 0.0 0.0 1.0 0.0 999.0"
# The same with VI, VA and the field voltage limits (VR) narrowed around the
# operating point, so the fault and the trip drive each into its limits.
const ESST1A_LIMITED =
    "1 1 0.0 0.1 0.006 2.5 13.25 0.0 0.0 200.0 0.1 3.0 1.8 2.6 1.85 0.0 0.0 1.0 0.0 999.0"
# tellegen's library PSS2B (src/rms/library.rs: the PSS2B of
# PowerSimulationsDynamics.jl 0.16.2's test/test_case53_pss2b.jl with its input
# limits open) with KS2 = T7 / (2 H) for the unit's H = 2.41 s.
pss2b_set(h) = join(
    [
        "1 0 3 0 5 1 1.5 1.5 0.0 1.5 0.0 1.5",
        @sprintf("%.4g", 1.5 / (2h)),
        "1.0 0.5 0.1 2.0 0.59451 0.0447 0.59451 0.0447 1.0 1.0 99.0 -99.0 99.0 -99.0 0.1 -0.1",
    ],
    " ",
)

function variants(base)
    g = base.params["GENROU"]
    sexs = base.params["SEXS"]
    tgov1 = base.params["TGOV1"]
    split_(s) = String.(split(s))
    pss2b = split_(pss2b_set(base.h))
    return [
        ("smib_genrou_esst1a_tgov1", ["GENROU" => g, "ESST1A" => split_(ESST1A_LIBRARY), "TGOV1" => tgov1]),
        ("smib_genrou_esst1a_lim_tgov1", ["GENROU" => g, "ESST1A" => split_(ESST1A_LIMITED), "TGOV1" => tgov1]),
        ("smib_genrou_pss2b_sexs_tgov1", ["GENROU" => g, "SEXS" => sexs, "TGOV1" => tgov1, "PSS2B" => pss2b]),
        (
            "smib_genrou_esst1a_pss2b_tgov1",
            ["GENROU" => g, "ESST1A" => split_(ESST1A_LIBRARY), "TGOV1" => tgov1, "PSS2B" => pss2b],
        ),
    ]
end

dyr_line(model, params) = "    102 '$model' 1 $(join(params, " ")) /\n"

"""The DYR text of a variant as tellegen reads it (every record)."""
dyr_text(recs) = "    101 'GENCLS' 1     0.0000       0.0000      /\n" * join(dyr_line(m, p) for (m, p) in recs)

# ---------------------------------------------------------------------------
# PowerSimulationsDynamics.jl
# ---------------------------------------------------------------------------

"""PowerSystems.jl's PSS2B from a DYR record's numbers (ICONs J1 K1 J2 K2 M
N, then the 23 CONs in PSS/E order). PowerSystems.jl 5.12.5 does not read
PSS2B records, so the stabilizer is attached by hand, as
PowerSimulationsDynamics.jl's own PSS2B test does."""
function pss2b_component(p)
    v = parse.(Float64, p)
    icon = round.(Int, v[1:6])
    c = v[7:end]
    return PSY.PSS2B(;
        input_code_1 = icon[1],
        remote_bus_control_1 = icon[2],
        input_code_2 = icon[3],
        remote_bus_control_2 = icon[4],
        M_rtf = icon[5],
        N_rtf = icon[6],
        Tw1 = c[1],
        Tw2 = c[2],
        T6 = c[3],
        Tw3 = c[4],
        Tw4 = c[5],
        T7 = c[6],
        Ks2 = c[7],
        Ks3 = c[8],
        T8 = c[9],
        T9 = c[10],
        Ks1 = c[11],
        T1 = c[12],
        T2 = c[13],
        T3 = c[14],
        T4 = c[15],
        T10 = c[16],
        T11 = c[17],
        Vs1_lim = (c[19], c[18]),
        Vs2_lim = (c[21], c[20]),
        Vst_lim = (c[23], c[22]),
    )
end

const UNIT = "generator-102-1"

"""A record as PowerSimulationsDynamics.jl is given it. ESST1A's second
lead-lag bypassed with TC1 = TB1 = 0 becomes TC1 = TB1 = 1: the same unit
transfer function (its state stays at zero), since PowerSimulationsDynamics.jl
divides by TB1 when it initializes the block (0/0 otherwise)."""
function psid_record(r)
    r.first == "ESST1A" || return r
    p = copy(r.second)
    if parse(Float64, p[8]) == 0.0 && parse(Float64, p[9]) == 0.0
        p[8] = "1.0"
        p[9] = "1.0"
    end
    return r.first => p
end

function build_system(dir, raw, recs)
    raw_path = joinpath(dir, "case.raw")
    dyr_path = joinpath(dir, "case.dyr")
    write(raw_path, raw)
    write(dyr_path, dyr_text([psid_record(r) for r in recs if r.first != "PSS2B"]))
    sys = with_logger(SimpleLogger(stderr, Logging.Error)) do
        System(raw_path, dyr_path; runchecks = false)
    end
    i = findfirst(r -> r.first == "PSS2B", recs)
    if i !== nothing
        gen = only(filter(g -> PSY.get_number(PSY.get_bus(g)) == 102, collect(get_components(Generator, sys))))
        old = get_dynamic_injector(gen)
        parts = (
            machine = deepcopy(get_machine(old)),
            shaft = deepcopy(get_shaft(old)),
            avr = deepcopy(get_avr(old)),
            prime_mover = deepcopy(get_prime_mover(old)),
            base_power = get_base_power(old),
        )
        remove_component!(sys, old)
        dyn = DynamicGenerator(;
            name = get_name(gen),
            ω_ref = 1.0,
            machine = parts.machine,
            shaft = parts.shaft,
            avr = parts.avr,
            prime_mover = parts.prime_mover,
            pss = pss2b_component(recs[i].second),
            base_power = parts.base_power,
        )
        add_component!(sys, dyn, gen)
    end
    return sys
end

function perturbations(sys, event, x_line)
    if event == "trip"
        # Circuit 2 of the two lines to the infinite bus.
        line = only(filter(l -> endswith(get_name(l), "-i_2"), collect(get_components(Line, sys))))
        return [BranchTrip(1.0, Line, get_name(line))]
    end
    @assert event == "fault"
    ybus = PNM.Ybus(sys; include_constant_impedance_loads = false)
    y = copy(ybus.data)
    k = PNM.get_lookup(ybus)[1][102]
    y[k, k] += 1 / (im * 0.5 * x_line)
    return [
        NetworkSwitch(1.0, SparseMatrixCSC{ComplexF32, Int}(y)),
        NetworkSwitch(1.1, SparseMatrixCSC{ComplexF32, Int}(copy(ybus.data))),
    ]
end

"""PSS2B's output from the saved states, block for block as
PowerSimulationsDynamics.jl's `mdl_pss_ode!` computes it (its post-processing
has no PSS2B output). The second input's washout only drives the T7 lag's
state, so the output needs neither it nor the electrical power."""
function pss2b_output(res, pss)
    @assert PSY.get_T7(pss) != 0.0 && PSY.get_T6(pss) == 0.0
    ts, ω = get_state_series(res, (UNIT, :ω))
    x = [get_state_series(res, (UNIT, Symbol("x_p$i")))[2] for i in 1:17]
    Vs1_min, Vs1_max = PSY.get_Vs1_lim(pss)
    Vst_min, Vst_max = PSY.get_Vst_lim(pss)
    Tw1, Tw2 = PSY.get_Tw1(pss), PSY.get_Tw2(pss)
    vs = similar(ω)
    for k in eachindex(ω)
        u1 = clamp(ω[k] - 1.0, Vs1_min, Vs1_max)
        y1, _ = PSID.high_pass(u1, x[1][k], Tw1, Tw1)
        y2 = Tw2 != 0.0 ? PSID.high_pass(y1, x[2][k], Tw2, Tw2)[1] : y1
        yt2 = x[6][k]
        y_rtf = PSID.ramp_tracking_filter(
            y2 + PSY.get_Ks3(pss) * yt2,
            (x[i][k] for i in 7:14)...,
            PSY.get_T9(pss),
            PSY.get_T8(pss),
            PSY.get_M_rtf(pss),
            PSY.get_N_rtf(pss),
        )[1]
        l1, _ = PSID.lead_lag(PSY.get_Ks1(pss) * (y_rtf - x[6][k]), x[15][k], 1.0, PSY.get_T1(pss), PSY.get_T2(pss))
        l2, _ = PSID.lead_lag(l1, x[16][k], 1.0, PSY.get_T3(pss), PSY.get_T4(pss))
        l3, _ = PSID.lead_lag(l2, x[17][k], 1.0, PSY.get_T10(pss), PSY.get_T11(pss))
        vs[k] = clamp(l3, Vst_min, Vst_max)
    end
    return ts, vs
end

"""GENROU's field current Xad·Ifd from the saved states, as the machine
model (`mdl_machine_ode!`) computes it. PowerSimulationsDynamics.jl 0.16.2's
own post-processing (`_field_current` in `post_proc_generator.jl`, and so
`get_field_current_series` and ESST1A's `get_field_voltage_series`) reads the
state ψ_kd where the model has ψ_kq, which only shows where the field
current matters: on ESST1A's upper field voltage limit `Vt VRMAX − KC Ifd`."""
function genrou_xad_ifd(res, machine)
    ts, δ = get_state_series(res, (UNIT, :δ))
    eq_p = get_state_series(res, (UNIT, :eq_p))[2]
    ed_p = get_state_series(res, (UNIT, :ed_p))[2]
    ψ_kd = get_state_series(res, (UNIT, :ψ_kd))[2]
    ψ_kq = get_state_series(res, (UNIT, :ψ_kq))[2]
    vm = get_voltage_magnitude_series(res, 102)[2]
    θ = get_voltage_angle_series(res, 102)[2]
    R = PSY.get_R(machine)
    Xd, Xd_p, Xd_pp, Xl = PSY.get_Xd(machine), PSY.get_Xd_p(machine), PSY.get_Xd_pp(machine), PSY.get_Xl(machine)
    Xq_pp = Xd_pp
    γ_d1, γ_q1, γ_d2 = PSY.get_γ_d1(machine), PSY.get_γ_q1(machine), PSY.get_γ_d2(machine)
    ifd = similar(δ)
    for k in eachindex(δ)
        V_dq = PSID.ri_dq(δ[k]) * [vm[k] * cos(θ[k]); vm[k] * sin(θ[k])]
        ψq_pp = γ_q1 * ed_p[k] + ψ_kq[k] * (1 - γ_q1)
        ψd_pp = γ_d1 * eq_p[k] + γ_d2 * (Xd_p - Xl) * ψ_kd[k]
        ψ_pp = sqrt(ψd_pp^2 + ψq_pp^2)
        I_d = (1.0 / (R^2 + Xq_pp * Xd_pp)) * (-R * (V_dq[1] - ψq_pp) + Xq_pp * (-V_dq[2] + ψd_pp))
        Se = PSID.saturation_function(machine, ψ_pp)
        ifd[k] = eq_p[k] + (Xd - Xd_p) * (γ_d1 * I_d - γ_d2 * ψ_kd[k] + γ_d2 * eq_p[k]) + Se * ψd_pp
    end
    return ts, ifd
end

"""ESST1A's field voltage and the signals at its limits, from the saved
states as `mdl_avr_ode!` computes them (VOS = 1, so without the stabilizer
signal): VI before its clamp, VA, the regulator output VA − ILIM and the
field voltage limits."""
function esst1a_signals(res, machine, avr, vref, vs)
    @assert PSY.get_PSS_flags(avr) == 1
    ts, vm = get_state_series(res, (UNIT, :Vm))
    _, va = get_state_series(res, (UNIT, :Va))
    _, vr3 = get_state_series(res, (UNIT, :Vr3))
    _, ifd = genrou_xad_ifd(res, machine)
    _, vt = get_voltage_magnitude_series(res, 102)
    Vr_min, Vr_max = PSY.get_Vr_lim(avr)
    Kc, K_lr, I_lr = PSY.get_Kc(avr), PSY.get_K_lr(avr), PSY.get_I_lr(avr)
    Kf, Tf = PSY.get_Kf(avr), PSY.get_Tf(avr)
    ilim = max.(K_lr .* (ifd .- I_lr), 0.0)
    va_sum = va .- ilim
    # At rest the field current equals the field voltage (Xad·Ifd = Efd).
    @assert abs(ifd[1] - va_sum[1]) < 1e-6
    y_hp = [PSID.high_pass(va_sum[k], vr3[k], Kf, Tf)[1] for k in eachindex(va)]
    lo = vt .* Vr_min
    hi = vt .* Vr_max .- Kc .* ifd
    efd = clamp.(va_sum, lo, hi)
    vi = vref .+ vs .- vm .- y_hp
    return (t = ts, va = va, va_sum = va_sum, efd = efd, lo = lo, hi = hi, vi = vi)
end

"""How long (s, 10 ms samples) each limit of the narrowed ESST1A holds."""
function limit_times(sig, avr)
    Vi_min, Vi_max = PSY.get_Vi_lim(avr)
    Va_min, Va_max = PSY.get_Va_lim(avr)
    on(mask) = round(0.01 * count(mask); digits = 2)
    return (
        VI_max = on(sig.vi .>= Vi_max),
        VI_min = on(sig.vi .<= Vi_min),
        VA_max = on(sig.va .>= Va_max - 1e-6),
        VA_min = on(sig.va .<= Va_min + 1e-6),
        Efd_max = on(sig.va_sum .>= sig.hi),
        Efd_min = on(sig.va_sum .<= sig.lo),
    )
end

sig7(v) = round.(v; sigdigits = 7)

function run_case(base, id, recs, event)
    dir = mktempdir()
    sys = build_system(dir, base.raw, recs)
    sim = with_logger(SimpleLogger(stderr, Logging.Error)) do
        Simulation(MassMatrixModel, sys, dir, (0.0, 3.0), perturbations(sys, event, base.x_line))
    end
    @assert sim.status == PSID.BUILT
    execute!(sim, Rodas4(); reltol = 1e-8, abstol = 1e-8, saveat = 0.01)
    res = read_results(sim)
    dyn = get_dynamic_injector(get_component(Generator, sys, UNIT))
    ts, _ = get_voltage_magnitude_series(res, 102)
    ch = Pair{String, Vector{Float64}}[]
    push!(ch, "va:101" => get_voltage_angle_series(res, 101)[2])
    push!(ch, "vm:102" => get_voltage_magnitude_series(res, 102)[2])
    push!(ch, "va:102" => get_voltage_angle_series(res, 102)[2])
    push!(ch, "omega:102" => get_state_series(res, (UNIT, :ω))[2])
    push!(ch, "delta:102" => get_state_series(res, (UNIT, :δ))[2])
    push!(ch, "pe:102" => get_activepower_series(res, UNIT)[2])
    push!(ch, "qe:102" => get_reactivepower_series(res, UNIT)[2])
    pss = get_pss(dyn)
    vs = zeros(length(ts))
    if pss isa PSY.PSS2B
        _, vs = pss2b_output(res, pss)
    end
    avr = get_avr(dyn)
    limits = nothing
    if avr isa PSY.ESST1A
        vref = get_setpoints(sim)[UNIT]["V_ref"]
        sig = esst1a_signals(res, get_machine(dyn), avr, vref, vs)
        push!(ch, "efd:102" => sig.efd)
        push!(ch, "sig:exciter.Va:102" => sig.va)
        occursin("_lim", id) && (limits = limit_times(sig, avr))
    else
        push!(ch, "efd:102" => get_field_voltage_series(res, UNIT)[2])
    end
    push!(ch, "tm:102" => get_mechanical_torque_series(res, UNIT)[2])
    pss isa PSY.PSS2B && push!(ch, "vs:102" => vs)
    # Every second sample: 20 ms.
    keep = [k for k in eachindex(ts) if isapprox(ts[k] / 0.02, round(ts[k] / 0.02); atol = 1e-6)]
    @assert length(keep) == 151
    time = round.(ts[keep]; digits = 2)
    channels = [k => sig7(v[keep]) for (k, v) in ch]
    return (time = time, channels = channels, limits = limits)
end

named(pairs) = NamedTuple{Tuple(Symbol(first(p)) for p in pairs)}(Tuple(last(p) for p in pairs))

const REFERENCE = "PowerSimulationsDynamics.jl 0.16.2, MassMatrixModel, Rodas4, reltol = abstol = 1.0e-8, saveat = 0.01; every second sample kept (20 ms)"

"""Tolerance per compared channel: three times tellegen's difference at a
1 ms step, at least 1e-6 (rounded up to four significant digits)."""
function tolerances(keys, measured)
    return named([k => (measured === nothing ? 1e-6 : max(1e-6, round(3 * measured[k], RoundUp; sigdigits = 4))) for k in keys])
end

function main(args)
    measured = isempty(args) ? nothing : JSON3.read(read(args[1], String), Dict{String, Dict{String, Dict{String, Float64}}})
    base = base_case()
    runs = []
    cases = Pair{String, Any}[]
    for (id, recs) in variants(base)
        push!(cases, id => (raw = base.raw, dyr = dyr_text(recs)))
        for event in ("trip", "fault")
            @info "$id $event"
            r = run_case(base, id, recs, event)
            r.limits === nothing || println("$id $event: time on limits (s) ", r.limits)
            keys_ = [k for (k, _) in r.channels if k != "va:101"]
            run_id = "$id $event"
            held = measured === nothing ? nothing : measured["held"][run_id]
            free = measured === nothing ? nothing : measured["free"][run_id]
            push!(
                runs,
                (
                    case = id,
                    event = event,
                    psid_lv_gain = false,
                    reference = REFERENCE,
                    time = r.time,
                    channels = named(r.channels),
                    tolerance = tolerances(keys_, held),
                    tolerance_free_limits = tolerances(keys_, free),
                ),
            )
        end
    end
    out = (
        about = "PowerSimulationsDynamics.jl 0.16.2 results (run as installed) for the SMIB extension model variants of crates/tellegen/tests/rms_psid_validation.rs (smib_ext_models_against_psid), generated by validation/rms-models/psid-smib/generate.jl; see SOURCES.md",
        cases = named(cases),
        smib = runs,
    )
    open(OUT, "w") do io
        JSON3.write(io, out)
    end
    println("wrote $OUT")
end

if abspath(PROGRAM_FILE) == @__FILE__
    main(ARGS)
end
