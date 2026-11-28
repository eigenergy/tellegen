# Reference for the load models: recovery, saturation, the one-sided rise of
# reactive power, and the PQBRAK breakpoint with and without its rounding,
# on a feeder with a classical machine. Writes `loads.json`.
#
#   julia --project=validation/rms-models validation/rms-models/reduced-network/loads.jl

include(joinpath(@__DIR__, "common.jl"))

# The feeder: an infinite source at 101 (GENCLS without inertia behind 1e-5 pu)
# feeds the load bus 103 over two lines of 0.1 pu; a classical machine at 102
# (80 MW at 1.02 pu, 100 MVA, X'd 0.2 pu, H 3 s, D 2 pu) feeds it over 0.05 pu;
# 103 feeds a second load bus 104 over 0.05 pu. Loads: 150 MW + 30 Mvar at 103,
# 60 MW + 20 Mvar at 104, as constant power.
const FEEDER = Case(
    [Bus(101, 3, 1.0), Bus(102, 2, 1.02), Bus(103, 1, 1.0), Bus(104, 1, 1.0)],
    [Gen(101, 0.0, 1.0, 100.0, 1e-5), Gen(102, 80.0, 1.02, 100.0, 0.2)],
    [Load(103, 150.0, 30.0), Load(104, 60.0, 20.0)],
    [Line(101, 103, "1", 0.1), Line(101, 103, "2", 0.1), Line(102, 103, "1", 0.05), Line(103, 104, "1", 0.05)],
    Dict(101 => (0.0, 0.0), 102 => (3.0, 2.0)),
)

const T_END = 4.0
const DT_OUT = 0.02

# The events, off the 20 ms sample grid.
const EVENTS = Dict(
    # A line from the source opens: a lasting dip, and the machine swings.
    "trip" => [(0.505, :trip, 101, 103, "1")],
    # 60 % of the load at 104 is shed: the voltages rise.
    "shed" => [(0.505, :scale, 104, 0.4)],
    # A fault at 103 through 0.05 pu for 100 ms: a dip to about 0.47 pu,
    # and swings above and below the start voltage after it.
    "fault" => [(0.505, :fault, 103, 0.05), (0.605, :clear, 103)],
    # A fault at 103 through 0.1 pu for 200 ms: 103 and 104 held near 0.69
    # pu, just below the PQBRAK break at 0.7 pu and within 0.1 pu of it.
    "fault_band" => [(0.505, :fault, 103, 0.1), (0.705, :clear, 103)],
)

const CHANNELS = [
    "vm:103", "vm:104", "va:103", "omega:102", "delta:102", "pe:102", "qe:102",
    "pl:103", "ql:103", "pl:104", "ql:104",
]

# The runs: name, event, switches.
const RUNS = [
    ("base", "trip", Switches()),
    ("base", "shed", Switches()),
    ("base", "fault", Switches()),
    ("recovery", "trip", Switches(alpha = 2.0, t_rec = 1.0)),
    ("recovery", "fault", Switches(alpha = 2.0, t_rec = 1.0)),
    ("saturation", "shed", Switches(sat_c = 2.0, sat_knee = 0.99)),
    ("saturation", "fault", Switches(sat_c = 2.0, sat_knee = 0.99)),
    ("rise", "shed", Switches(rise_c = 2.0)),
    ("rise", "fault", Switches(rise_c = 2.0)),
    ("pqbrak", "fault_band", Switches(lv = :break, v_break = 0.7)),
    ("pqbrak_smooth", "fault_band", Switches(lv = :break, v_break = 0.7, smooth = 0.1)),
]

function switches_json(sw::Switches)
    d = Dict{String, Any}()
    sw.alpha > 0 && (d["load_recovery"] = [sw.alpha, sw.t_rec])
    sw.sat_c > 0 && (d["load_saturation"] = [sw.sat_c, sw.sat_knee])
    sw.rise_c > 0 && (d["load_rise"] = sw.rise_c)
    sw.lv == :break && (d["v_break"] = sw.v_break)
    sw.smooth > 0 && (d["pqbrak_smooth"] = sw.smooth)
    return d
end

"""
The model here without a switch (`chs`, the channels of its run through
`event`) against PowerSimulationsDynamics.jl 0.16.2 (unmodified) on the same
feeder: the largest difference of each channel.
"""
function psid_check(event, chs)
    sys = psid_system(FEEDER)
    perts = psid_perturbations(sys, EVENTS[event]; load_part = :power)
    sim, res = psid_run(sys, T_END, DT_OUT, perts)
    gen = "generator-102-1"
    series = Dict(
        "vm:103" => get_voltage_magnitude_series(res, 103),
        "vm:104" => get_voltage_magnitude_series(res, 104),
        "va:103" => get_voltage_angle_series(res, 103),
        "omega:102" => get_state_series(res, (gen, :ω)),
        "delta:102" => get_state_series(res, (gen, :δ)),
        "pe:102" => get_activepower_series(res, gen),
        "qe:102" => get_reactivepower_series(res, gen),
    )
    grid = collect(0:DT_OUT:T_END+1e-9)
    worst = Dict{String, Float64}()
    for (k, (t, x)) in series
        worst[k] = maximum(abs.(psid_on_grid(t, x, grid) .- [c[k] for c in chs]))
    end
    println("  PowerSimulationsDynamics.jl against the model, $event: ",
        join(["$k $(@sprintf("%.1e", v))" for (k, v) in sort(collect(worst))], ", "))
    return worst
end

function main()
    runs = []
    check = Dict{String, Any}()
    for (name, event, sw) in RUNS
        m, u0 = build(FEEDER, sw)
        t0 = time()
        ts, us, chs = simulate(m, u0, T_END, DT_OUT, EVENTS[event])
        v103 = [c["vm:103"] for c in chs]
        println(@sprintf("%-14s %-6s V103 %.4f..%.4f (start %.4f), %.1f s", name, event,
            minimum(v103), maximum(v103), v103[1], time() - t0))
        run = Dict{String, Any}(
            "name" => name,
            "event" => event,
            "switches" => switches_json(sw),
            "time" => ts,
            "channels" => export_channels(chs, CHANNELS),
        )
        if name == "base"
            check[event] = psid_check(event, chs)
        end
        push!(runs, run)
    end
    write_json("loads.json", Dict(
        "case" => "feeder",
        "psid_check" => check,
        "events" => Dict(k => [[string(x) for x in e] for e in v] for (k, v) in EVENTS),
        "runs" => runs,
    ))
end

if abspath(PROGRAM_FILE) == @__FILE__
    main()
end
