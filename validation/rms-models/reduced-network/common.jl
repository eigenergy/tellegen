# Shared pieces of the reduced-network references.
#
# A case is a small network: buses, generators, constant-power loads and
# lossless lines on 100 MVA and 60 Hz, as a PSS/E RAW file states them, and per
# generator a classical machine (GENCLS) or, without inertia, a voltage source
# behind its ZSORCE reactance.
#
# The model here is written from the documented equations:
#
# - network: the bus current balance `I_inj(V) = Y V` in rectangular
#   coordinates, `Y` from the series reactances of the lines in service plus
#   the fault admittances;
# - classical machine (PowerSimulationsDynamics.jl `BaseMachine` + `SingleMass`):
#   a constant EMF `e'q` at the rotor angle `δ` behind `jX'd` (machine base),
#   `dδ/dt = Ωb (ω − ω_sys)`, `2H dω/dt = Pm − Pe − D (ω − 1) / ω` with `Pe`
#   the power at the EMF and `Pm` constant;
# - voltage source: a constant EMF behind `jX_th` (`ω_sys = 1` with one);
# - loads: per bus a constant-power demand `P0 + jQ0` (power flow values,
#   scaled by load changes), drawn as `I = (P − jQ) / conj(V)` with the laws of
#   `load_power` below;
# - HVDC links in AC emulation: per link the active power
#   `clamp(K (Δθ − Δθ0), −Pmax, Pmax)` drawn at the from bus and injected at
#   the to bus, `Δθ` the angle of the from bus against the to bus and `Δθ0`
#   its start value.
#
# The model starts from its own Newton power flow (generators at their
# voltage setpoints, no reactive limits) and is integrated with Rodas5P at
# reltol = abstol = 1e-10 on the mass-matrix form, every sample time a forced
# step (tstops), and every event ends a segment: the algebraic variables are
# solved again by Newton (to the rounding floor) with the differential states held, and the
# integration restarts.

using LinearAlgebra
using Printf
using OrdinaryDiffEq
using JSON3
using PowerSimulationsDynamics
using PowerSystems
const PSID = PowerSimulationsDynamics
const PSY = PowerSystems
const FD = PSID.ForwardDiff

const SBASE = 100.0
const ΩB = 2π * 60.0
const DATA_DIR = normpath(
    joinpath(@__DIR__, "..", "..", "..", "crates", "tellegen", "tests", "data", "rms_reduced_network"),
)
const RELTOL = 1e-10
const ABSTOL = 1e-10

# ---------------------------------------------------------------------------
# Cases.

struct Bus
    num::Int
    kind::Int   # 1 load, 2 generator, 3 slack
    v::Float64
end

struct Gen
    bus::Int
    pg::Float64     # MW
    vs::Float64     # pu
    mbase::Float64  # MVA
    zx::Float64     # ZSORCE reactance, pu of MBASE
end

struct Load
    bus::Int
    p::Float64  # MW
    q::Float64  # Mvar
end

struct Line
    f::Int
    t::Int
    ckt::String
    x::Float64
end

struct Case
    buses::Vector{Bus}
    gens::Vector{Gen}
    loads::Vector{Load}
    lines::Vector{Line}
    # bus => (H, D) of its classical machine; H = 0: a voltage source.
    machines::Dict{Int, Tuple{Float64, Float64}}
end

"""The PSS/E RAW text of a case (version 32, 100 MVA, 60 Hz)."""
function raw_text(c::Case)
    io = IOBuffer()
    println(io, "0,   100.00,  32, 0, 1, 60.00     / reduced network reference case")
    println(io)
    println(io)
    for b in c.buses
        @printf(io, "%d,'B%d', 230.0000,%d,   1,   1,   1,%.5f,   0.0000\n", b.num, b.num, b.kind, b.v)
    end
    println(io, " 0 /End of Bus data, Begin Load data")
    for l in c.loads
        @printf(io, "%d,'1 ',1,   1,   1, %.3f, %.3f, 0.000, 0.000, 0.000, 0.000,   1,1\n", l.bus, l.p, l.q)
    end
    println(io, " 0 /End of Load data, Begin Fixed shunt data")
    println(io, " 0 /End of Fixed shunt data, Begin Generator data")
    for g in c.gens
        @printf(
            io,
            "%d,'1 ', %.3f, 0.000, 9999.000, -9999.000,%.5f, 0, %.3f, 0.00000E+0, %.5E, 0.00000E+0, 0.00000E+0,1.00000,1,  100.0, 9999.000, -9999.000,   1,1.0000\n",
            g.bus, g.pg, g.vs, g.mbase, g.zx
        )
    end
    println(io, " 0 /End of Generator data, Begin Branch data")
    for l in c.lines
        @printf(
            io,
            "%d, %d,'%s ', 0.00000E+0, %.5E, 0.00000, 9999.00, 9999.00, 9999.00, 0.00000, 0.00000, 0.00000, 0.00000,1,1, 0.00, 1,1.0000\n",
            l.f, l.t, l.ckt, l.x
        )
    end
    print(io, RAW_TAIL)
    return String(take!(io))
end

const RAW_TAIL = """ 0 /End of Branch data, Begin Transformer data
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

"""The DYR text of a case's machines (GENCLS H D per generator)."""
function dyr_text(c::Case)
    io = IOBuffer()
    for g in c.gens
        h, d = c.machines[g.bus]
        @printf(io, "%d 'GENCLS' 1 %.4f %.4f /\n", g.bus, h, d)
    end
    return String(take!(io))
end

# ---------------------------------------------------------------------------
# Power flow.

bus_index(c::Case) = Dict(b.num => i for (i, b) in enumerate(c.buses))

"""The bus admittance matrix of the lines in service (`active[k]` for line k)."""
function ybus(c::Case, active::AbstractVector{Bool})
    idx = bus_index(c)
    n = length(c.buses)
    Y = zeros(ComplexF64, n, n)
    for (k, l) in enumerate(c.lines)
        active[k] || continue
        y = 1 / (im * l.x)
        i, j = idx[l.f], idx[l.t]
        Y[i, i] += y
        Y[j, j] += y
        Y[i, j] -= y
        Y[j, i] -= y
    end
    return Y
end

"""
Newton power flow: the slack bus at its generator's setpoint and angle 0,
generator buses at their setpoints with their scheduled active power, load
buses with their constant-power demand. Returns the complex bus voltages.
"""
function power_flow(c::Case)
    idx = bus_index(c)
    n = length(c.buses)
    Y = ybus(c, trues(length(c.lines)))
    p = zeros(n)
    q = zeros(n)
    vset = [b.v for b in c.buses]
    for g in c.gens
        p[idx[g.bus]] += g.pg / SBASE
        vset[idx[g.bus]] = g.vs
    end
    for l in c.loads
        p[idx[l.bus]] -= l.p / SBASE
        q[idx[l.bus]] -= l.q / SBASE
    end
    kinds = [b.kind for b in c.buses]
    ang = findall(!=(3), kinds)
    mag = findall(==(1), kinds)
    function voltages(z)
        θ = zeros(eltype(z), n)
        v = convert(Vector{eltype(z)}, copy(vset))
        θ[ang] = z[1:length(ang)]
        v[mag] = z[length(ang)+1:end]
        return v .* cis.(θ)
    end
    function mismatch(z)
        V = voltages(z)
        S = V .* conj.(Y * V)
        return vcat(real.(S[ang]) .- p[ang], imag.(S[mag]) .- q[mag])
    end
    z = vcat(zeros(length(ang)), ones(length(mag)))
    for _ in 1:50
        r = mismatch(z)
        maximum(abs, r) < 1e-14 && break
        z -= FD.jacobian(mismatch, z) \ r
    end
    @assert maximum(abs, mismatch(z)) < 1e-12
    return voltages(z)
end

# ---------------------------------------------------------------------------
# The differential-algebraic model.

"""The model switches of the loads and links (all off by default)."""
Base.@kwdef mutable struct Switches
    # The law of the loads: :power (constant power) or :impedance (constant
    # admittance at the power flow voltage, P0 (V / V0)^2 and Q0 (V / V0)^2).
    load_model::Symbol = :power
    # Load recovery: P = P0 (V / Vf)^alpha, dVf/dt = (V − Vf) / t_rec.
    alpha::Float64 = 0.0
    t_rec::Float64 = 0.0
    # Load saturation: extra Q = c P0 ((V − Vk) / 0.1)^2 above Vk = max(knee, V0).
    sat_c::Float64 = 0.0
    sat_knee::Float64 = 0.0
    # One-sided rise: extra Q = c P0 ((V − V0) / 0.1)^2 above V0.
    rise_c::Float64 = 0.0
    # Low-voltage behaviour of the constant-power law: :exact, or :break at
    # `v_break` (capped at V0), rounded off over ±`smooth` when the bus starts
    # at least `smooth` above its break.
    lv::Symbol = :exact
    v_break::Float64 = 0.7
    smooth::Float64 = 0.0
    # HVDC links in AC emulation: (from bus, to bus, K MW/degree, limit MW, 0 = none).
    links::Vector{Tuple{Int, Int, Float64, Float64}} = Tuple{Int, Int, Float64, Float64}[]
end

mutable struct Model
    case::Case
    sw::Switches
    n::Int
    idx::Dict{Int, Int}
    active::Vector{Bool}
    fault_y::Vector{ComplexF64}
    G::Matrix{Float64}
    B::Matrix{Float64}
    # Machines: bus index, H, D, X'd (machine base), MBASE, e'q, Pm (machine base).
    mach::Vector{NamedTuple{(:b, :h, :d, :x, :mbase, :eq, :pm), Tuple{Int, Float64, Float64, Float64, Float64, Float64, Float64}}}
    # Sources: bus index, EMF, X_th (system base).
    src::Vector{NamedTuple{(:b, :e, :x), Tuple{Int, ComplexF64, Float64}}}
    # Loads: bus index, P0, Q0 (system base, power flow values), V0, scale.
    load::Vector{NamedTuple{(:b, :p0, :q0, :v0), Tuple{Int, Float64, Float64, Float64}}}
    scale::Vector{Float64}
    # Links: from, to (bus indices), K pu/rad, Pmax pu, Δθ0.
    link::Vector{NamedTuple{(:f, :t, :k, :pmax, :dθ0), Tuple{Int, Int, Float64, Float64, Float64}}}
    # Index of the machine whose speed is the frame (0: a source holds it at 1).
    ref::Int
    nx::Int
end

function update_y!(m::Model)
    Y = ybus(m.case, m.active)
    for i in 1:m.n
        Y[i, i] += m.fault_y[i]
    end
    m.G = real.(Y)
    m.B = imag.(Y)
    return m
end

recovering(m::Model) = m.sw.alpha > 0 && m.sw.t_rec > 0

"""
Power a load draws at the voltage magnitude `v` (and the recovery reference
`vf`): its constant-power law under the low-voltage choice, the recovery of
its active power, and the extra reactive power of saturation and rise.
"""
function load_power(m::Model, k::Int, v, vf)
    l = m.load[k]
    s = m.sw
    p0 = l.p0 * m.scale[k]
    q0 = l.q0 * m.scale[k]
    v0 = l.v0
    if s.load_model == :impedance
        @assert !recovering(m) && s.sat_c == 0 && s.rise_c == 0
        return p0 * (v / v0)^2, q0 * (v / v0)^2
    end
    if s.lv == :exact
        @assert v > 0.2 * v0 "the exact law holds above 20 % of V0 (V = $v)"
        p = recovering(m) ? p0 * (v / vf)^s.alpha : p0
        q = q0
    else
        @assert !recovering(m)
        vb = min(s.v_break, v0)
        d = s.smooth
        u = if d > 0 && v0 - vb >= d && abs(v - vb) < d
            vb + (v - vb + d)^2 / (4d)
        else
            max(v, vb)
        end
        # Constant power above the break, the admittance of its power at the
        # break below it: P0 (V / u)^2.
        p = p0 * (v / u)^2
        q = q0 * (v / u)^2
    end
    if s.sat_c > 0 && p0 > 0
        vk = max(s.sat_knee, v0)
        if v > vk
            q += s.sat_c * p0 * ((v - vk) / 0.1)^2
        end
    end
    if s.rise_c > 0 && p0 > 0 && v > v0
        q += s.rise_c * p0 * ((v - v0) / 0.1)^2
    end
    return p, q
end

link_angle(vrf, vif, vrt, vit) = atan(vif * vrt - vrf * vit, vrf * vrt + vif * vit)

function link_power(m::Model, k::Int, vr, vi)
    l = m.link[k]
    dθ = link_angle(vr[l.f], vi[l.f], vr[l.t], vi[l.t])
    return clamp(l.k * (dθ - l.dθ0), -l.pmax, l.pmax)
end

"""Builds the model at the power flow point; returns it and the start state."""
function build(c::Case, sw::Switches)
    idx = bus_index(c)
    n = length(c.buses)
    V = power_flow(c)
    Y = ybus(c, trues(length(c.lines)))
    # Bus injections of the generators: what the network and the loads take.
    s_load = zeros(ComplexF64, n)
    for l in c.loads
        s_load[idx[l.bus]] += (l.p + im * l.q) / SBASE
    end
    s_inj = V .* conj.(Y * V) .+ s_load
    mach = []
    src = []
    ref = 0
    xm = Float64[]
    for g in c.gens
        b = idx[g.bus]
        h, d = c.machines[g.bus]
        i_sys = conj(s_inj[b] / V[b])
        if h == 0
            x = g.zx * SBASE / g.mbase
            push!(src, (b = b, e = V[b] + im * x * i_sys, x = x))
        else
            i_m = i_sys * SBASE / g.mbase
            e = V[b] + im * g.zx * i_m
            push!(mach, (b = b, h = h, d = d, x = g.zx, mbase = g.mbase, eq = abs(e), pm = real(e * conj(i_m))))
            append!(xm, [angle(e), 1.0])
            if c.buses[b].kind == 3
                ref = length(mach)
            end
        end
    end
    isempty(src) || (ref = 0)
    load = [(b = idx[l.bus], p0 = l.p / SBASE, q0 = l.q / SBASE, v0 = abs(V[idx[l.bus]])) for l in c.loads]
    link = [
        (
            f = idx[f], t = idx[t], k = kmw * 180 / π / SBASE, pmax = pmw > 0 ? pmw / SBASE : Inf,
            dθ0 = link_angle(real(V[idx[f]]), imag(V[idx[f]]), real(V[idx[t]]), imag(V[idx[t]])),
        ) for (f, t, kmw, pmw) in sw.links
    ]
    m = Model(
        c, sw, n, idx, trues(length(c.lines)), zeros(ComplexF64, n), zeros(n, n), zeros(n, n),
        [mach...], [src...], [load...], ones(length(load)), [link...], ref, 0,
    )
    if recovering(m)
        append!(xm, [l.v0 for l in load])
    end
    m.nx = length(xm)
    update_y!(m)
    u0 = vcat(xm, real.(V), imag.(V))
    return m, u0
end

function rhs!(du, u, m::Model, t)
    T = eltype(u)
    n = m.n
    vr = @view u[m.nx+1:m.nx+n]
    vi = @view u[m.nx+n+1:m.nx+2n]
    ir = zeros(T, n)
    ii = zeros(T, n)
    ωsys = m.ref == 0 ? one(T) : u[2m.ref]
    for (k, g) in enumerate(m.mach)
        δ, ω = u[2k-1], u[2k]
        er, ei = g.eq * cos(δ), g.eq * sin(δ)
        # I = (E − V) / (j X), machine base.
        imr = (ei - vi[g.b]) / g.x
        imi = -(er - vr[g.b]) / g.x
        pe = er * imr + ei * imi
        du[2k-1] = ΩB * (ω - ωsys)
        du[2k] = (g.pm - pe - g.d * (ω - 1) / ω) / (2g.h)
        ir[g.b] += imr * g.mbase / SBASE
        ii[g.b] += imi * g.mbase / SBASE
    end
    for s in m.src
        ir[s.b] += (imag(s.e) - vi[s.b]) / s.x
        ii[s.b] += -(real(s.e) - vr[s.b]) / s.x
    end
    nm = 2 * length(m.mach)
    for (k, l) in enumerate(m.load)
        b = l.b
        v2 = vr[b]^2 + vi[b]^2
        v = sqrt(v2)
        vf = recovering(m) ? u[nm+k] : one(T)
        p, q = load_power(m, k, v, vf)
        ir[b] -= (p * vr[b] + q * vi[b]) / v2
        ii[b] -= (p * vi[b] - q * vr[b]) / v2
        if recovering(m)
            du[nm+k] = (v - vf) / m.sw.t_rec
        end
    end
    for (k, l) in enumerate(m.link)
        p = link_power(m, k, vr, vi)
        mf = vr[l.f]^2 + vi[l.f]^2
        mt = vr[l.t]^2 + vi[l.t]^2
        ir[l.f] -= p * vr[l.f] / mf
        ii[l.f] -= p * vi[l.f] / mf
        ir[l.t] += p * vr[l.t] / mt
        ii[l.t] += p * vi[l.t] / mt
    end
    du[m.nx+1:m.nx+n] .= ir .- (m.G * vr .- m.B * vi)
    du[m.nx+n+1:m.nx+2n] .= ii .- (m.G * vi .+ m.B * vr)
    return nothing
end

function residual(m::Model, u)
    du = similar(u)
    rhs!(du, u, m, 0.0)
    return du
end

"""Solves the algebraic variables again with the differential states held."""
function solve_algebraic!(m::Model, u)
    x = u[1:m.nx]
    g(y) = residual(m, vcat(convert(Vector{eltype(y)}, x), y))[m.nx+1:end]
    y = u[m.nx+1:end]
    for _ in 1:50
        r = g(y)
        maximum(abs, r) < 1e-10 && break
        y -= FD.jacobian(g, y) \ r
    end
    # One more step: the residual's floor is set by the source admittance (1e5).
    y -= FD.jacobian(g, y) \ g(y)
    @assert maximum(abs, g(y)) < 1e-9 "algebraic re-solve failed"
    u[m.nx+1:end] .= y
    return u
end

# Events: (time, kind, args...).
#   (:trip, from, to, ckt)  open a line
#   (:fault, bus, x)        a fault to ground through j x (system pu)
#   (:clear, bus)           remove it
#   (:scale, bus, s)        scale the load at a bus (P and Q)
# Only events between samples, so every sample is a state of the integration.
function apply!(m::Model, ev)
    kind = ev[2]
    if kind == :trip
        k = findfirst(l -> l.f == ev[3] && l.t == ev[4] && l.ckt == ev[5], m.case.lines)
        m.active[k] = false
    elseif kind == :fault
        m.fault_y[m.idx[ev[3]]] = 1 / (im * ev[4])
    elseif kind == :clear
        m.fault_y[m.idx[ev[3]]] = 0
    elseif kind == :scale
        for (k, l) in enumerate(m.load)
            l.b == m.idx[ev[3]] && (m.scale[k] *= ev[4])
        end
    else
        error("unknown event $kind")
    end
    update_y!(m)
end

"""
Integrates the model to `t_end` through `events`, sampled every `dt_out`
seconds (no event may fall on a sample time). Returns the sample times, the
states there and their channels (taken with the network and loads of their
time).
"""
function simulate(m::Model, u0, t_end, dt_out, events)
    grid = collect(0:dt_out:t_end+1e-9)
    times = sort(unique([e[1] for e in events]))
    @assert all(t -> minimum(abs.(grid .- t)) > 1e-6, times) "an event falls on a sample time"
    @assert maximum(abs, residual(m, u0)) < 1e-10 "the start is not a steady state"
    bounds = vcat(0.0, times, t_end)
    M = Diagonal(vcat(ones(m.nx), zeros(2m.n)))
    f = ODEFunction(rhs!; mass_matrix = M)
    out = Dict{Float64, Vector{Float64}}(0.0 => copy(u0))
    chs = Dict{Float64, Dict{String, Float64}}(0.0 => channels(m, u0))
    u = copy(u0)
    for s in 1:length(bounds)-1
        ta, tb = bounds[s], bounds[s+1]
        pts = filter(t -> t > ta && t <= tb + 1e-12, grid)
        prob = ODEProblem(f, u, (ta, tb), m)
        sol = solve(prob, Rodas5P(); reltol = RELTOL, abstol = ABSTOL, tstops = pts, saveat = pts,
            save_start = false, save_end = true, initializealg = NoInit(), maxiters = 10^7)
        @assert SciMLBase.successful_retcode(sol) "integration failed at $(sol.t[end]): $(sol.retcode)"
        for (t, x) in zip(sol.t, sol.u)
            out[t] = x
            chs[t] = channels(m, x)
        end
        u = copy(sol.u[end])
        for e in events
            e[1] == tb && apply!(m, e)
        end
        tb < t_end && solve_algebraic!(m, u)
    end
    ks = collect(keys(out))
    @assert all(t -> minimum(abs.(ks .- t)) < 1e-9, grid) "a sample is missing"
    at(t) = ks[argmin(abs.(ks .- t))]
    return grid, [out[at(t)] for t in grid], [chs[at(t)] for t in grid]
end

"""The channels of a state, named as tellegen names them."""
function channels(m::Model, u)
    c = m.case
    n = m.n
    vr = u[m.nx+1:m.nx+n]
    vi = u[m.nx+n+1:m.nx+2n]
    V = vr .+ im .* vi
    ch = Dict{String, Float64}()
    for (i, b) in enumerate(c.buses)
        ch["vm:$(b.num)"] = abs(V[i])
        ch["va:$(b.num)"] = angle(V[i])
    end
    for (k, g) in enumerate(m.mach)
        num = c.buses[g.b].num
        δ, ω = u[2k-1], u[2k]
        e = g.eq * cis(δ)
        i_m = (e - V[g.b]) / (im * g.x)
        i_sys = i_m * g.mbase / SBASE
        s = V[g.b] * conj(i_sys)
        ch["delta:$num"] = δ
        ch["omega:$num"] = ω
        ch["pe:$num"] = real(e * conj(i_sys))
        ch["qe:$num"] = imag(s)
    end
    nm = 2 * length(m.mach)
    for (k, l) in enumerate(m.load)
        num = c.buses[l.b].num
        vf = recovering(m) ? u[nm+k] : 1.0
        p, q = load_power(m, k, abs(V[l.b]), vf)
        ch["pl:$num"] = get(ch, "pl:$num", 0.0) + p
        ch["ql:$num"] = get(ch, "ql:$num", 0.0) + q
    end
    for (k, l) in enumerate(c.lines)
        i, j = m.idx[l.f], m.idx[l.t]
        key = l.ckt == "1" ? "$(l.f)-$(l.t)" : "$(l.f)-$(l.t)#$(l.ckt)"
        if m.active[k]
            s = V[i] * conj((V[i] - V[j]) / (im * l.x))
            ch["p_from:$key"] = real(s)
        else
            ch["p_from:$key"] = 0.0
        end
    end
    for (k, l) in enumerate(m.link)
        ch["link:$(c.buses[l.f].num)-$(c.buses[l.t].num)"] = link_power(m, k, vr, vi)
    end
    return ch
end

"""Eigenvalues of the model linearized at `u` (algebraic variables eliminated)."""
function eigenvalues(m::Model, u)
    J = FD.jacobian(x -> residual(m, x), u)
    nx = m.nx
    xx, xy = J[1:nx, 1:nx], J[1:nx, nx+1:end]
    yx, yy = J[nx+1:end, 1:nx], J[nx+1:end, nx+1:end]
    return eigvals(xx - xy * (yy \ yx))
end

# ---------------------------------------------------------------------------
# PowerSimulationsDynamics.jl runs of the same case.

"""The PowerSystems.jl system of a case, from its RAW and DYR text."""
function psid_system(c::Case; dyr = dyr_text(c))
    dir = mktempdir()
    raw_path = joinpath(dir, "case.raw")
    dyr_path = joinpath(dir, "case.dyr")
    write(raw_path, raw_text(c))
    write(dyr_path, dyr)
    return PSY.System(raw_path, dyr_path; runchecks = false)
end

function psid_line(sys, f, t, ckt)
    for l in PSY.get_components(PSY.Line, sys)
        a = PSY.get_arc(l)
        nf, nt = PSY.get_number(PSY.get_from(a)), PSY.get_number(PSY.get_to(a))
        if nf == f && nt == t && endswith(PSY.get_name(l), "_$ckt")
            return l
        end
    end
    error("no line $f-$t ($ckt)")
end

psid_load(sys, bus) =
    only(filter(l -> PSY.get_number(PSY.get_bus(l)) == bus, collect(PSY.get_components(PSY.StandardLoad, sys))))

"""
The perturbations of `events` for PowerSimulationsDynamics.jl: line trips and
faults as `NetworkSwitch` of the admittance matrix (`BranchTrip` would take
out both of two parallel circuits), load scales as `LoadChange` of the load's
constant-power (`:power`) or constant-impedance (`:impedance`) part.
"""
function psid_perturbations(sys, events; load_part = :power)
    # The admittance matrix of the lines alone, in the bus order of the
    # simulation (PowerNetworkMatrices.jl's would also hold loads as
    # constant impedance, which the simulation draws as devices).
    lookup = PSID.PNM.Ybus(sys).lookup[1]
    y = zeros(ComplexF64, length(lookup), length(lookup))
    for l in PSY.get_components(PSY.Line, sys)
        a = PSY.get_arc(l)
        i, j = lookup[PSY.get_number(PSY.get_from(a))], lookup[PSY.get_number(PSY.get_to(a))]
        yl = 1 / (PSY.get_r(l) + im * PSY.get_x(l))
        y[i, i] += yl
        y[j, j] += yl
        y[i, j] -= yl
        y[j, i] -= yl
    end
    perts = PSID.Perturbation[]
    scales = Dict{Int, Float64}()
    for e in sort(events; by = first)
        t, kind = e[1], e[2]
        if kind in (:trip, :fault, :clear)
            if kind == :trip
                x = PSY.get_x(psid_line(sys, e[3], e[4], e[5]))
                i, j = lookup[e[3]], lookup[e[4]]
                yl = 1 / (im * x)
                y[i, i] -= yl
                y[j, j] -= yl
                y[i, j] += yl
                y[j, i] += yl
            else
                k = lookup[e[3]]
                yf = 1 / (im * (kind == :fault ? e[4] : fault_x[e[3]]))
                kind == :fault && (fault_x[e[3]] = e[4])
                y[k, k] += kind == :fault ? yf : -yf
            end
            push!(perts, NetworkSwitch(t, PSID.SparseArrays.sparse(ComplexF32.(y))))
        elseif kind == :scale
            ld = psid_load(sys, e[3])
            s = get(scales, e[3], 1.0) * e[4]
            scales[e[3]] = s
            if load_part == :power
                push!(perts, LoadChange(t, ld, :P_ref_power, s * PSY.get_constant_active_power(ld)))
                push!(perts, LoadChange(t, ld, :Q_ref_power, s * PSY.get_constant_reactive_power(ld)))
            else
                push!(perts, LoadChange(t, ld, :P_ref_impedance, s * PSY.get_impedance_active_power(ld)))
                push!(perts, LoadChange(t, ld, :Q_ref_impedance, s * PSY.get_impedance_reactive_power(ld)))
            end
        else
            error("unknown event $kind")
        end
    end
    return perts
end
const fault_x = Dict{Int, Float64}()

"""
Runs a PowerSimulationsDynamics.jl simulation (MassMatrixModel, Rodas4 at
reltol = abstol = 1e-8, as the single-machine comparison) and returns it and
its results. The algebraic states are solved again after every event
(`BrownFullBasicInit`; by default PowerSimulationsDynamics.jl does not, which
its Rosenbrock steps absorb only while the network is linear).
"""
function psid_run(sys, t_end, dt_out, perts)
    path = mktempdir()
    sim = Simulation(MassMatrixModel, sys, path, (0.0, t_end), perts)
    grid = collect(0:dt_out:t_end+1e-9)
    st = execute!(sim, Rodas4(); reltol = 1e-8, abstol = 1e-8, saveat = grid,
        initializealg = BrownFullBasicInit())
    @assert st == PSID.SIMULATION_FINALIZED "PowerSimulationsDynamics.jl: $st"
    return sim, read_results(sim)
end

"""A PowerSimulationsDynamics.jl series on the sample grid."""
function psid_on_grid(t, x, grid)
    return [x[findmin(abs.(t .- g))[2]] for g in grid]
end

# ---------------------------------------------------------------------------
# Output.

r7(x) = round(x; sigdigits = 7)

"""Every channel of `keys` over the run, rounded to seven significant digits."""
function export_channels(chs, keys)
    return Dict(k => [r7(c[k]) for c in chs] for k in keys)
end

function write_json(name, value)
    mkpath(DATA_DIR)
    path = joinpath(DATA_DIR, name)
    open(path, "w") do io
        JSON3.write(io, value)
        println(io)
    end
    println("wrote ", path, " (", filesize(path), " bytes)")
end
