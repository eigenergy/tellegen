# Controller models written for this reference, as PowerSimulationsDynamics.jl
# extensions: new PowerSystems.jl component types (subtypes of AVR and
# TurbineGov) with the model and initialization methods that
# PowerSimulationsDynamics.jl dispatches on. The machine, shaft, network and
# solver are PowerSimulationsDynamics.jl's own, unmodified; no function of it
# is overridden.
#
# The equations come from IEEE Std 421.5-2005 (AC6A, the field current
# limiter and the load compensator), the PSS/E model descriptions (SEXS,
# GAST) and, for the models that exist only in tellegen (OELIT, UELPQ, the
# summing-point under-excitation limiter, the compensator's lag and the GAST
# valve rate limit), the doc comments that define them. See SOURCES.md in
# crates/tellegen/tests/data/rms_reduced_controllers.

using PowerSystems
using PowerSimulationsDynamics
const PSY = PowerSystems
const PSID = PowerSimulationsDynamics
const IS = PSY.IS

# ---------------------------------------------------------------------------
# Elementary blocks, written out here (time constants positive).

"Lead-lag `K (1 + s T1) / (1 + s T2)` with state `x`: output and dx/dt."
function leadlag(u, x, K, T1, T2)
    if T2 <= 0.0
        return K * u, zero(x)
    end
    return K * ((T1 / T2) * u + (1.0 - T1 / T2) * x), (u - x) / T2
end

"Lag `K / (1 + s T)` with a non-windup limit on its state: output and dx/dt."
function lag_nonwindup(u, x, K, T, lo, hi)
    dx = (K * u - x) / T
    if (x >= hi && dx > 0.0) || (x <= lo && dx < 0.0)
        dx = zero(dx)
    end
    return clamp(x, lo, hi), dx
end

"PSS/E quadratic saturation `B (x - A)^2 / x` above `A`, zero below."
quad_se(A, B, x) = (B == 0.0 || x <= A) ? zero(x) : B * (x - A)^2 / x

"Coefficients `(A, B)` of the quadratic saturation through `(E1, SE1)`, `(E2, SE2)`."
function quad_coeffs(E1, SE1, E2, SE2)
    (E1 == 0.0 || E2 == 0.0 || SE1 == 0.0 || SE2 == 0.0) && return (0.0, 0.0)
    # sqrt(SE x / B) = x - A at both points.
    r = sqrt(SE2 * E2 / (SE1 * E1))
    A = (r * E1 - E2) / (r - 1.0)
    B = SE2 * E2 / (E2 - A)^2
    return (A, B)
end

"Rectifier regulation `FEX(IN)` of IEEE Std 421.5-2005, Annex D."
function fex(IN)
    if IN <= 0.0
        return one(IN)
    elseif IN <= 0.433
        return 1.0 - 0.577 * IN
    elseif IN < 0.75
        return sqrt(0.75 - IN^2)
    elseif IN <= 1.0
        return 1.732 * (1.0 - IN)
    else
        return zero(IN)
    end
end

# ---------------------------------------------------------------------------
# The machine's stator current, machine base, network (R, I) frame. Every
# machine here is PowerSimulationsDynamics.jl's GENROU (RoundRotorQuadratic);
# these are its stator equations (`_update_inner_vars!`), evaluated at the
# states of the step, so the regulator sees the current the machine injects.

function stator_current(device_states, inner_vars, dynamic_device)
    machine = PSY.get_machine(dynamic_device)
    machine isa PSY.RoundRotorQuadratic || error("stator_current: GENROU only")
    M = typeof(machine)
    x = @view device_states[PSID.get_local_state_ix(dynamic_device, M)]
    eq_p, ed_p, ψ_kd, ψ_kq = x[1], x[2], x[3], x[4]
    δ = device_states[PSID.get_input_port_ix(dynamic_device, M)[1]]
    V_R = inner_vars[PSID.VR_gen_var]
    V_I = inner_vars[PSID.VI_gen_var]
    R = PSY.get_R(machine)
    Xd_p = PSY.get_Xd_p(machine)
    Xd_pp = PSY.get_Xd_pp(machine)
    Xl = PSY.get_Xl(machine)
    γ_d1 = PSY.get_γ_d1(machine)
    γ_q1 = PSY.get_γ_q1(machine)
    γ_d2 = PSY.get_γ_d2(machine)
    s, c = sin(δ), cos(δ)
    V_d = s * V_R - c * V_I
    V_q = c * V_R + s * V_I
    ψq_pp = γ_q1 * ed_p + ψ_kq * (1 - γ_q1)
    ψd_pp = γ_d1 * eq_p + γ_d2 * (Xd_p - Xl) * ψ_kd
    den = R^2 + Xd_pp * Xd_pp
    I_d = (1.0 / den) * (-R * (V_d - ψq_pp) + Xd_pp * (-V_q + ψd_pp))
    I_q = (1.0 / den) * (Xd_pp * (V_d - ψq_pp) + R * (-V_q + ψd_pp))
    I_R = s * I_d + c * I_q
    I_I = -c * I_d + s * I_q
    return I_R, I_I
end

"Terminal voltage, stator current, P and Q (machine base) at the step."
function terminal(device_states, inner_vars, dynamic_device)
    V_R = inner_vars[PSID.VR_gen_var]
    V_I = inner_vars[PSID.VI_gen_var]
    I_R, I_I = stator_current(device_states, inner_vars, dynamic_device)
    P = V_R * I_R + V_I * I_I
    Q = V_I * I_R - V_R * I_I
    return V_R, V_I, I_R, I_I, P, Q
end

# ---------------------------------------------------------------------------
# Summing-point limiters (the OELIT and UELPQ records of tellegen).

"Reset time of the limiters' dead-zone integrators, s."
const LIMITER_RESET = 1.0

"""
The dead-zone PI on a violation `e`: output `clamp(KP max(0, e) + x, 0, xmax)`,
`dx/dt = KI min(max(e, -x/(KI TR)), (xmax - x)/(KI TR))`.
"""
function dead_zone_pi(e, x, KP, KI, xmax)
    c = 1.0 / (max(KI, 1e-9) * LIMITER_RESET)
    dx = KI * min(max(e, -c * x), c * (xmax - x))
    return clamp(KP * max(e, 0.0) + x, 0.0, xmax), dx
end

"""
OELIT: output VOEL (<= 0), the field current limit in effect, and the
derivatives of its heating `acc` and integrator `x`. `p` = IFDRAT IPICK ICONT ILIM C TCOOL KP KI VOELMIN.
"""
function oelit(p, Ifd, acc, x)
    IFDRAT, IPICK, ICONT, ILIM, C, TCOOL, KP, KI, VOELMIN = p
    heat = max((Ifd / (IPICK * IFDRAT))^2 - 1.0, 0.0)
    dacc = heat - acc / TCOOL
    s = clamp((acc - 0.95 * C) / (0.05 * C), 0.0, 1.0)
    limit = ILIM * IFDRAT - (ILIM - ICONT) * IFDRAT * s
    out, dx = dead_zone_pi(Ifd - limit, x, KP, KI, -VOELMIN)
    return -out, limit, dacc, dx
end

"""
UELPQ: output VUEL (>= 0) and the derivative of its integrator. `p` = Q0 KQP
KV KP KI VUELMAX; the limit is `(Q0 + KQP P) Vt^KV`.
"""
function uelpq(p, P, Q, Vt, x)
    Q0, KQP, KV, KP, KI, VUELMAX = p
    qlim = (Q0 + KQP * P) * Vt^round(Int, KV)
    return dead_zone_pi(qlim - Q, x, KP, KI, VUELMAX)
end

# ---------------------------------------------------------------------------
# ESAC6A (IEEE Std 421.5-2005, Figure 6-6) with the AC exciter of ESAC1A, and
# optionally OELIT and UELPQ at its summing point.

mutable struct RefESAC6A <: PSY.AVR
    p::Vector{Float64}  # the 23 CONs of the DYR record
    oel::Union{Nothing, Vector{Float64}}
    uel::Union{Nothing, Vector{Float64}}
    V_ref::Float64
    ext::Dict{String, Any}
    states::Vector{Symbol}
    n_states::Int
    states_types::Vector{PSY.StateTypes}
    internal::IS.InfrastructureSystemsInternal
end

const ESAC6A_STATES = [:a6_Vm, :a6_xa, :a6_xb, :a6_xh, :a6_Ve, :a6_acc, :a6_xo, :a6_xu]

function RefESAC6A(p; oel = nothing, uel = nothing)
    length(p) == 23 || error("ESAC6A: 23 constants")
    n = length(ESAC6A_STATES)
    return RefESAC6A(
        p, oel, uel, 1.0, Dict{String, Any}(), ESAC6A_STATES, n,
        fill(PSY.StateTypes.Differential, n), IS.InfrastructureSystemsInternal(),
    )
end

PSY.get_V_ref(a::RefESAC6A) = a.V_ref
PSY.set_V_ref!(a::RefESAC6A, v) = (a.V_ref = v)
PSY.get_ext(a::RefESAC6A) = a.ext
PSY.get_states(a::RefESAC6A) = a.states
PSY.get_n_states(a::RefESAC6A) = a.n_states
PSY.get_states_types(a::RefESAC6A) = a.states_types
PSY.get_internal(a::RefESAC6A) = a.internal

"Every signal of the ESAC6A at states `x` and inputs; derivatives in `dx`."
function esac6a_eval(a::RefESAC6A, x, V_ref, Vs, Vt, Ifd, P, Q)
    TR, KA, TA, TK, TB, TC, VAMAX, VAMIN, VRMAX, VRMIN, TE, VFELIM, KH, VHMAX, TH, TJ,
    KC, KD, KE, E1, SE1, E2, SE2 = a.p
    A, B = quad_coeffs(E1, SE1, E2, SE2)
    Vm, xa, xb, xh, Ve, acc, xo, xu = x
    T = promote_type(eltype(x), typeof(Vt), typeof(Ifd))
    VOEL, limit, dacc, dxo = zero(T), zero(T), zero(T), zero(T)
    if a.oel !== nothing
        VOEL, limit, dacc, dxo = oelit(a.oel, Ifd, acc, xo)
    end
    VUEL, dxu = zero(T), zero(T)
    if a.uel !== nothing
        VUEL, dxu = uelpq(a.uel, P, Q, Vt, xu)
    end
    dVm = (Vt - Vm) / TR
    err = V_ref + Vs + VOEL + VUEL - Vm
    VA_ll, dxa = leadlag(err, xa, KA, TK, TA)
    VA = clamp(VA_ll, VAMIN, VAMAX)
    VR_ll, dxb = leadlag(VA, xb, 1.0, TC, TB)
    VFE = KD * Ifd + KE * Ve + quad_se(A, B, Ve) * Ve
    VH_ll, dxh = leadlag(VFE - VFELIM, xh, KH, TJ, TH)
    VH = clamp(VH_ll, 0.0, VHMAX)
    VR = clamp(VR_ll - VH, Vt * VRMIN, Vt * VRMAX)
    dVe = (VR - VFE) / TE
    Efd = Ve * fex(KC * Ifd / max(Ve, 1e-3))
    dx = (dVm, dxa, dxb, dxh, dVe, dacc, dxo, dxu)
    sig = (
        Efd = Efd, Vm = Vm, Va = VA, VR = VR, VH = VH, VFE = VFE, Ve = Ve,
        VOEL = VOEL, oel_acc = acc, oel_limit = limit, oel_x = xo, VUEL = VUEL, uel_x = xu,
    )
    return dx, sig
end

function PSID.mdl_avr_ode!(
    device_states::AbstractArray{<:PSID.ACCEPTED_REAL_TYPES},
    output_ode::AbstractArray{<:PSID.ACCEPTED_REAL_TYPES},
    inner_vars::AbstractArray{<:PSID.ACCEPTED_REAL_TYPES},
    dynamic_device::PSID.DynamicWrapper{PSY.DynamicGenerator{M, S, RefESAC6A, TG, P}},
    h,
    t,
) where {M <: PSY.Machine, S <: PSY.Shaft, TG <: PSY.TurbineGov, P <: PSY.PSS}
    a = PSY.get_avr(dynamic_device)
    ix = PSID.get_local_state_ix(dynamic_device, RefESAC6A)
    x = @view device_states[ix]
    V_R, V_I, _, _, Pg, Qg = terminal(device_states, inner_vars, dynamic_device)
    Vt = sqrt(V_R^2 + V_I^2)
    dx, sig = esac6a_eval(
        a, x, PSID.get_V_ref(dynamic_device), inner_vars[PSID.V_pss_var], Vt,
        inner_vars[PSID.Xad_Ifd_var], Pg, Qg,
    )
    for k in eachindex(ix)
        output_ode[ix[k]] = dx[k]
    end
    inner_vars[PSID.Vf_var] = sig.Efd
    return
end

"The `Ve` that gives `Efd` at field current `Ifd` (bisection on `Ve FEX`)."
function solve_ve(Efd, Ifd, KC)
    lo, hi = Efd, Efd + KC * Ifd + 1.0
    for _ in 1:200
        mid = 0.5 * (lo + hi)
        mid * fex(KC * Ifd / mid) < Efd ? (lo = mid) : (hi = mid)
    end
    return 0.5 * (lo + hi)
end

function PSID.initialize_avr!(
    device_states,
    static::PSY.StaticInjection,
    dynamic_device::PSID.DynamicWrapper{PSY.DynamicGenerator{M, S, RefESAC6A, TG, P}},
    inner_vars::AbstractVector,
) where {M <: PSY.Machine, S <: PSY.Shaft, TG <: PSY.TurbineGov, P <: PSY.PSS}
    a = PSY.get_avr(dynamic_device)
    TR, KA, TA, TK, TB, TC, VAMAX, VAMIN, VRMAX, VRMIN, TE, VFELIM, KH, VHMAX, TH, TJ,
    KC, KD, KE, E1, SE1, E2, SE2 = a.p
    A, B = quad_coeffs(E1, SE1, E2, SE2)
    Efd0 = inner_vars[PSID.Vf_var]
    Ifd0 = inner_vars[PSID.Xad_Ifd_var]
    Vt0 = sqrt(inner_vars[PSID.VR_gen_var]^2 + inner_vars[PSID.VI_gen_var]^2)
    Vs0 = inner_vars[PSID.V_pss_var]
    Ve0 = solve_ve(Efd0, Ifd0, KC)
    VFE0 = KD * Ifd0 + KE * Ve0 + quad_se(A, B, Ve0) * Ve0
    VH0 = clamp(KH * (VFE0 - VFELIM), 0.0, VHMAX)
    VR0 = VFE0
    VA0 = VR0 + VH0
    (Vt0 * VRMIN <= VR0 <= Vt0 * VRMAX) || error("ESAC6A: VR at rest outside its limits")
    (VAMIN <= VA0 <= VAMAX) || error("ESAC6A: VA at rest outside its limits")
    err0 = VA0 / KA
    V_ref = Vt0 + err0 - Vs0
    ix = PSID.get_local_state_ix(dynamic_device, RefESAC6A)
    x = @view device_states[ix]
    x .= (Vt0, err0, VA0, VFE0 - VFELIM, Ve0, 0.0, 0.0, 0.0)
    # The limiters start inactive.
    _, _, _, _, P0, Q0 = terminal(device_states, inner_vars, dynamic_device)
    _, sig = esac6a_eval(a, x, V_ref, Vs0, Vt0, Ifd0, P0, Q0)
    (sig.VOEL == 0.0 && sig.VUEL == 0.0) || error("ESAC6A: a limiter is active at rest")
    PSY.set_V_ref!(a, V_ref)
    PSID.set_V_ref(dynamic_device, V_ref)
    return
end

# ---------------------------------------------------------------------------
# SEXS (PSS/E) with tellegen's AVR load compensator and summing-point
# under-excitation limiter.

mutable struct RefSEXS <: PSY.AVR
    p::Vector{Float64}  # TA/TB TB K TE EMIN EMAX
    vcomp::Union{Nothing, NTuple{3, Float64}}  # Rc Xc T (T > 0: lagged droop)
    uel::Union{Nothing, NTuple{2, Float64}}  # q_lim gain
    V_ref::Float64
    ext::Dict{String, Any}
    states::Vector{Symbol}
    n_states::Int
    states_types::Vector{PSY.StateTypes}
    internal::IS.InfrastructureSystemsInternal
end

const SEXS_STATES = [:sx_Efd, :sx_xr, :sx_z]

function RefSEXS(p; vcomp = nothing, uel = nothing)
    n = length(SEXS_STATES)
    return RefSEXS(
        p, vcomp, uel, 1.0, Dict{String, Any}(), SEXS_STATES, n,
        fill(PSY.StateTypes.Differential, n), IS.InfrastructureSystemsInternal(),
    )
end

PSY.get_V_ref(a::RefSEXS) = a.V_ref
PSY.set_V_ref!(a::RefSEXS, v) = (a.V_ref = v)
PSY.get_ext(a::RefSEXS) = a.ext
PSY.get_states(a::RefSEXS) = a.states
PSY.get_n_states(a::RefSEXS) = a.n_states
PSY.get_states_types(a::RefSEXS) = a.states_types
PSY.get_internal(a::RefSEXS) = a.internal

"""
The voltage the regulator sees: `|Vt + (Rc + j Xc) It|` (IEEE Std 421.5-2005,
Figure 3-1), or with a lag `T` on the reactive current `z` (`T dz/dt = Q/Vt - z`)
`|Vt + Rc It| + Xc z`.
"""
function compensated(vcomp, V_R, V_I, I_R, I_I, z)
    vcomp === nothing && return sqrt(V_R^2 + V_I^2)
    Rc, Xc, T = vcomp
    if T > 0.0
        return sqrt((V_R + Rc * I_R)^2 + (V_I + Rc * I_I)^2) + Xc * z
    end
    return sqrt((V_R + Rc * I_R - Xc * I_I)^2 + (V_I + Rc * I_I + Xc * I_R)^2)
end

"The summing-point UEL: `gain max(0, -q_lim Vt^2 - Q)`."
uel_sum(uel, Vt, Q) = uel === nothing ? zero(Q) : uel[2] * max(-uel[1] * Vt^2 - Q, 0.0)

function sexs_eval(a::RefSEXS, x, V_ref, Vs, V_R, V_I, I_R, I_I, Q)
    Ta_Tb, Tb, K, Te, Emin, Emax = a.p
    Efd, xr, z = x
    Vt = sqrt(V_R^2 + V_I^2)
    Vc = compensated(a.vcomp, V_R, V_I, I_R, I_I, z)
    u = V_ref + Vs + uel_sum(a.uel, Vt, Q) - Vc
    y, dxr = leadlag(u, xr, 1.0, Ta_Tb * Tb, Tb)
    Efd_out, dEfd = lag_nonwindup(y, Efd, K, Te, Emin, Emax)
    dz = zero(dxr)
    if a.vcomp !== nothing && a.vcomp[3] > 0.0
        dz = (Q / max(Vt, 1e-3) - z) / a.vcomp[3]
    end
    return (dEfd, dxr, dz), Efd_out, Vc
end

function PSID.mdl_avr_ode!(
    device_states::AbstractArray{<:PSID.ACCEPTED_REAL_TYPES},
    output_ode::AbstractArray{<:PSID.ACCEPTED_REAL_TYPES},
    inner_vars::AbstractArray{<:PSID.ACCEPTED_REAL_TYPES},
    dynamic_device::PSID.DynamicWrapper{PSY.DynamicGenerator{M, S, RefSEXS, TG, P}},
    h,
    t,
) where {M <: PSY.Machine, S <: PSY.Shaft, TG <: PSY.TurbineGov, P <: PSY.PSS}
    a = PSY.get_avr(dynamic_device)
    ix = PSID.get_local_state_ix(dynamic_device, RefSEXS)
    x = @view device_states[ix]
    V_R, V_I, I_R, I_I, _, Q = terminal(device_states, inner_vars, dynamic_device)
    dx, Efd, _ = sexs_eval(
        a, x, PSID.get_V_ref(dynamic_device), inner_vars[PSID.V_pss_var],
        V_R, V_I, I_R, I_I, Q,
    )
    for k in eachindex(ix)
        output_ode[ix[k]] = dx[k]
    end
    inner_vars[PSID.Vf_var] = Efd
    return
end

function PSID.initialize_avr!(
    device_states,
    static::PSY.StaticInjection,
    dynamic_device::PSID.DynamicWrapper{PSY.DynamicGenerator{M, S, RefSEXS, TG, P}},
    inner_vars::AbstractVector,
) where {M <: PSY.Machine, S <: PSY.Shaft, TG <: PSY.TurbineGov, P <: PSY.PSS}
    a = PSY.get_avr(dynamic_device)
    Ta_Tb, Tb, K, Te, Emin, Emax = a.p
    Efd0 = inner_vars[PSID.Vf_var]
    (Emin <= Efd0 <= Emax) || error("SEXS: Efd at rest outside its limits")
    Vs0 = inner_vars[PSID.V_pss_var]
    V_R, V_I, I_R, I_I, _, Q0 = terminal(device_states, inner_vars, dynamic_device)
    Vt0 = sqrt(V_R^2 + V_I^2)
    z0 = Q0 / Vt0
    Vc0 = compensated(a.vcomp, V_R, V_I, I_R, I_I, z0)
    uel_sum(a.uel, Vt0, Q0) == 0.0 || error("SEXS: the UEL is active at rest")
    V_ref = Vc0 + Efd0 / K - Vs0
    ix = PSID.get_local_state_ix(dynamic_device, RefSEXS)
    device_states[ix] .= (Efd0, Efd0 / K, z0)
    PSY.set_V_ref!(a, V_ref)
    PSID.set_V_ref(dynamic_device, V_ref)
    return
end

# ---------------------------------------------------------------------------
# GAST (PSS/E, after Rowen 1983) with tellegen's fuel valve rate limit.

mutable struct RefGAST <: PSY.TurbineGov
    p::Vector{Float64}  # R T1 T2 T3 AT KT VMAX VMIN DTURB
    rate::Float64  # largest valve speed, pu/s (0: none)
    P_ref::Float64
    ext::Dict{String, Any}
    states::Vector{Symbol}
    n_states::Int
    states_types::Vector{PSY.StateTypes}
    internal::IS.InfrastructureSystemsInternal
end

const GAST_STATES = [:gt_x1, :gt_x2, :gt_x3]

function RefGAST(p; rate = 0.0)
    n = length(GAST_STATES)
    return RefGAST(
        p, rate, 1.0, Dict{String, Any}(), GAST_STATES, n,
        fill(PSY.StateTypes.Differential, n), IS.InfrastructureSystemsInternal(),
    )
end

PSY.get_P_ref(g::RefGAST) = g.P_ref
PSY.set_P_ref!(g::RefGAST, v) = (g.P_ref = v)
PSY.get_ext(g::RefGAST) = g.ext
PSY.get_states(g::RefGAST) = g.states
PSY.get_n_states(g::RefGAST) = g.n_states
PSY.get_states_types(g::RefGAST) = g.states_types
PSY.get_internal(g::RefGAST) = g.internal

"""
GAST: the fuel demand `min(P_ref - Δω/R, AT + KT (AT - x3))` through the valve
lag `1/(1 + s T1)` (non-windup limits VMIN, VMAX; its speed at most `rate`),
the turbine lag `1/(1 + s T2)` (power `x2 - DTURB Δω`) and the exhaust
temperature lag `1/(1 + s T3)`.
"""
function gast_eval(g::RefGAST, x, P_ref, ω)
    R, T1, T2, T3, AT, KT, VMAX, VMIN, DTURB = g.p
    x1, x2, x3 = x
    inv_R = R < eps() ? 0.0 : 1.0 / R
    x_in = min(P_ref - inv_R * (ω - 1.0), AT + KT * (AT - x3))
    v, dx1 = lag_nonwindup(x_in, x1, 1.0, T1, VMIN, VMAX)
    if g.rate > 0.0
        dx1 = clamp(dx1, -g.rate, g.rate)
    end
    dx2 = (v - x2) / T2
    dx3 = (x2 - x3) / T3
    Pm = x2 - DTURB * (ω - 1.0)
    return (dx1, dx2, dx3), Pm
end

function PSID.mdl_tg_ode!(
    device_states::AbstractArray{<:PSID.ACCEPTED_REAL_TYPES},
    output_ode::AbstractArray{<:PSID.ACCEPTED_REAL_TYPES},
    inner_vars::AbstractArray{<:PSID.ACCEPTED_REAL_TYPES},
    ω_sys::PSID.ACCEPTED_REAL_TYPES,
    device::PSID.DynamicWrapper{PSY.DynamicGenerator{M, S, A, RefGAST, P}},
    h,
    t,
) where {M <: PSY.Machine, S <: PSY.Shaft, A <: PSY.AVR, P <: PSY.PSS}
    g = PSY.get_prime_mover(device)
    ix = PSID.get_local_state_ix(device, RefGAST)
    ω = device_states[PSID.get_input_port_ix(device, RefGAST)[1]]
    dx, Pm = gast_eval(g, @view(device_states[ix]), PSID.get_P_ref(device), ω)
    for k in eachindex(ix)
        output_ode[ix[k]] = dx[k]
    end
    inner_vars[PSID.τm_var] = Pm / ω
    return
end

function PSID.initialize_tg!(
    device_states,
    static::PSY.StaticInjection,
    dynamic_device::PSID.DynamicWrapper{PSY.DynamicGenerator{M, S, A, RefGAST, P}},
    inner_vars::AbstractVector,
) where {M <: PSY.Machine, S <: PSY.Shaft, A <: PSY.AVR, P <: PSY.PSS}
    g = PSY.get_prime_mover(dynamic_device)
    R, T1, T2, T3, AT, KT, VMAX, VMIN, DTURB = g.p
    τm0 = inner_vars[PSID.τm_var]
    (VMIN <= τm0 <= VMAX && τm0 < AT) || error("GAST: dispatch outside its limits")
    ix = PSID.get_local_state_ix(dynamic_device, RefGAST)
    device_states[ix] .= τm0
    PSY.set_P_ref!(g, τm0)
    PSID.set_P_ref(dynamic_device, τm0)
    return
end
