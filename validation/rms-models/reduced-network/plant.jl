# Reference for the voltage exponent of a renewable plant's reactive power
# set point: PowerSimulationsDynamics.jl 0.16.2 runs the plant feeder, its
# REPCA1 reactive path extended by the exponent. Writes `plant.json`.
#
#   julia --project=validation/rms-models validation/rms-models/reduced-network/plant.jl
#
# The extension: in reactive power control (Ref_Flag = 0, PF_Flag = 0,
# V_Flag = 1) the plant controller regulates its filtered reactive power to
# the set point `Q_ref (V / V0)^n` instead of `Q_ref`, `V` the plant's
# measured terminal voltage (the REECB1 `Trv` filter, `Vt_filt`) and `V0` its
# value at the start. It is a method of
# `_mdl_ode_RE_reactive_controller_AB!` for the plant of this case (the REGCA1
# converter with the REECB1 inner control), which dispatch prefers to the
# general one: the body of PowerSimulationsDynamics.jl 0.16.2's method for
# these flags (`src/models/inverter_models/outer_control_models.jl`,
# BSD-3-Clause, Copyright (c) 2020, 2023 Alliance for Sustainable Energy, LLC
# and The Regents of the University of California) with the set point line
# changed. With n = 0 it is that method.

include(joinpath(@__DIR__, "common.jl"))

# The exponent and the start voltage the extension reads.
const Q_EXPONENT = Ref(0.0)
const Q_V0 = Ref(1.0)

const PlantWrapper = PSID.DynamicWrapper{
    PSY.DynamicInverter{
        PSY.RenewableEnergyConverterTypeA,
        PSY.OuterControl{PSY.ActiveRenewableControllerAB, PSY.ReactiveRenewableControllerAB},
        PSY.RECurrentControlB,
        PSY.FixedDCSource,
        PSY.FixedFrequency,
        PSY.RLFilter,
        Nothing,
    },
}

function PSID._mdl_ode_RE_reactive_controller_AB!(
    reactive_controller_ode::AbstractArray{<:PSID.ACCEPTED_REAL_TYPES},
    reactive_controller_states::AbstractArray{<:PSID.ACCEPTED_REAL_TYPES},
    q_elec_out::PSID.ACCEPTED_REAL_TYPES,
    Vt_filt::PSID.ACCEPTED_REAL_TYPES,
    ::Val{0},
    ::Val{0},
    ::Val{1},
    reactive_power_control::PSY.ReactiveRenewableControllerAB,
    dynamic_device::PlantWrapper,
    inner_vars::AbstractVector,
)
    # The set point, following the measured voltage with the exponent.
    q_ref = PSID.get_Q_ref(dynamic_device) * (Vt_filt / Q_V0[])^Q_EXPONENT[]

    # From here on PowerSimulationsDynamics.jl 0.16.2's method.
    T_fltr = PSY.get_T_fltr(reactive_power_control)
    K_p = PSY.get_K_p(reactive_power_control)
    K_i = PSY.get_K_i(reactive_power_control)
    T_ft = PSY.get_T_ft(reactive_power_control)
    T_fv = PSY.get_T_fv(reactive_power_control)
    e_min, e_max = PSY.get_e_lim(reactive_power_control)
    dbd1, dbd2 = PSY.get_dbd_pnts(reactive_power_control)
    Q_min, Q_max = PSY.get_Q_lim(reactive_power_control)
    Q_min_inner, Q_max_inner = PSY.get_Q_lim_inner(reactive_power_control)
    V_min, V_max = PSY.get_V_lim(reactive_power_control)
    K_qp = PSY.get_K_qp(reactive_power_control)
    K_qi = PSY.get_K_qi(reactive_power_control)

    q_flt = reactive_controller_states[1]
    ξq_oc = reactive_controller_states[2]
    q_LL = reactive_controller_states[3]
    ξ_Q = reactive_controller_states[4]

    q_err = clamp(PSID.deadband_function(q_ref - q_flt, dbd1, dbd2), e_min, e_max)

    _, dqflt_dt = PSID.low_pass(q_elec_out, q_flt, 1.0, T_fltr)
    Q_pi_sat, dξqoc_dt = PSID.pi_block_nonwindup(q_err, ξq_oc, K_p, K_i, Q_min, Q_max)
    Q_ext, dqLL_dt = PSID.lead_lag(Q_pi_sat, q_LL, 1.0, T_ft, T_fv)

    V_pi_in = clamp(Q_ext, Q_min_inner, Q_max_inner) - q_elec_out
    V_pi_sat, dξQ_dt = PSID.pi_block_nonwindup(V_pi_in, ξ_Q, K_qp, K_qi, V_min, V_max)

    reactive_controller_ode[1] = dqflt_dt
    reactive_controller_ode[2] = dξqoc_dt
    reactive_controller_ode[3] = dqLL_dt
    reactive_controller_ode[4] = dξQ_dt

    inner_vars[PSID.V_oc_var] = V_pi_sat - Vt_filt
    inner_vars[PSID.Iq_oc_var] = Q_ext / max(Vt_filt, PSID.VOLTAGE_DIVISION_LOWER_BOUND)
    return
end

# The plant feeder: the infinite source at 101 feeds the load bus 103
# (150 MW + 30 Mvar, constant impedance at its power flow voltage) over two
# lines of 0.1 pu; the plant at 104 (50 MW on 100 MVA at 1.01 pu) feeds it
# over 0.05 pu.
const PLANT_FEEDER = Case(
    [Bus(101, 3, 1.0), Bus(103, 1, 1.0), Bus(104, 2, 1.01)],
    [Gen(101, 0.0, 1.0, 100.0, 1e-5), Gen(104, 50.0, 1.01, 100.0, 0.2)],
    [Load(103, 150.0, 30.0)],
    [Line(101, 103, "1", 0.1), Line(101, 103, "2", 0.1), Line(104, 103, "1", 0.05)],
    Dict(101 => (0.0, 0.0)),
)

# The plant (REGCA1, REECB1, REPCA1) in reactive power control: the plant
# controller's PI (Kp 0.5, Ki 5, no deadband) sets the reactive current
# command.
const PLANT_DYR = """
101 'GENCLS' 1 0.0 0.0 /
104 'REGCA1' 1 1 0.02 10.0 0.9 0.4 1.22 1.2 0.9 0.5 -1.3 0.02 0.0 100.0 -100.0 0.7 /
104 'REECB1' 1 0 0 1 0 0 -99.0 99.0 0.02 0.0 0.0 0.0 1.1 -1.1 0.0 0.05 0.44 -0.44 1.05 0.9 0.0 0.01 10.0 60.0 0.02 99.0 -99.0 1.0 0.0 1.11 0.02 /
104 'REPCA1' 1 0 0 0 '0 ' 0 0 1 0.02 0.5 5.0 0.0 0.05 0.0 0.0 0.0 0.02 0.1 -0.1 0.0 0.0 0.43 -0.43 1.0 0.05 0.25 -1.0 1.0 99.0 -99.0 1.0 0.0 0.1 0.0 0.0 /
"""

const T_END = 5.0
const DT_OUT = 0.02

const EVENTS = Dict(
    # The load at 103 up by half: a lasting dip.
    "step" => [(0.505, :scale, 103, 1.5)],
    # A line from the source opens.
    "trip" => [(0.505, :trip, 101, 103, "1")],
)

const RUNS = [("base", "step", 0.0), ("q_exponent", "step", 2.0), ("q_exponent", "trip", 2.0)]

const CHANNELS = ["vm:103", "vm:104", "va:104", "pe:104", "qe:104"]

function main()
    runs = []
    for (name, event, n) in RUNS
        sys = psid_system(PLANT_FEEDER; dyr = PLANT_DYR)
        for l in PSY.get_components(PSY.StandardLoad, sys)
            transform_load_to_constant_impedance(l)
        end
        inv = only(PSY.get_components(PSY.DynamicInverter, sys))
        rc = PSY.get_reactive_power_control(PSY.get_outer_control(inv))
        @assert (PSY.get_Ref_Flag(rc), PSY.get_PF_Flag(rc), PSY.get_V_Flag(rc)) == (0, 0, 1)
        Q_EXPONENT[] = n
        Q_V0[] = 1.01
        perts = psid_perturbations(sys, EVENTS[event]; load_part = :impedance)
        t0 = time()
        sim, res = psid_run(sys, T_END, DT_OUT, perts)
        # The start voltage the extension divides by is the plant's
        # measured voltage at the start.
        vt0 = read_initial_conditions(sim)
        grid = collect(0:DT_OUT:T_END+1e-9)
        gen = "generator-104-1"
        series = Dict(
            "vm:103" => get_voltage_magnitude_series(res, 103),
            "vm:104" => get_voltage_magnitude_series(res, 104),
            "va:104" => get_voltage_angle_series(res, 104),
            "pe:104" => get_activepower_series(res, gen),
            "qe:104" => get_reactivepower_series(res, gen),
        )
        chs = Dict(k => r7.(psid_on_grid(t, x, grid)) for (k, (t, x)) in series)
        @assert abs(chs["vm:104"][1] - Q_V0[]) < 1e-7
        q = chs["qe:104"]
        v = chs["vm:104"]
        println(@sprintf("%-10s %-4s n %.0f: Q %.5f -> %.5f, V104 %.5f -> %.5f, Q0 (V/V0)^n %.5f, %.1f s",
            name, event, n, q[1], q[end], v[1], v[end], q[1] * (v[end] / v[1])^n, time() - t0))
        sw = n == 0 ? Dict() : Dict("plant_q_exponent" => Dict("104" => n))
        push!(runs, Dict("name" => name, "event" => event, "switches" => sw, "time" => grid, "channels" => chs))
    end
    write_json("plant.json", Dict("case" => "plant_feeder", "runs" => runs))
end

if abspath(PROGRAM_FILE) == @__FILE__
    main()
end
