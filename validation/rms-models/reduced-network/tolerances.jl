# Writes the tolerances into the reference files from tellegen's measured
# differences: per compared channel (and per inter-area mode) three times
# tellegen's difference at the 1 ms step, at least 1e-6, rounded up to two
# significant digits.
#
#   RMS_REDUCED_MEASURE=1 cargo test -p tellegen --features rms-experimental \
#       --test rms_reduced_network -- --nocapture > measured.txt
#   julia --project=validation/rms-models validation/rms-models/reduced-network/tolerances.jl measured.txt

using JSON3

const DATA_DIR = normpath(
    joinpath(@__DIR__, "..", "..", "..", "crates", "tellegen", "tests", "data", "rms_reduced_network"),
)

tolerance(d) = max(round(3d, RoundUp; sigdigits = 2), 1e-6)

function main(measured)
    files = Dict{String, Any}()
    load(f) = get!(() -> copy(JSON3.read(read(joinpath(DATA_DIR, f), String), Dict{String, Any})), files, f)
    for line in eachline(measured)
        startswith(line, "{") || continue
        m = JSON3.read(line, Dict{String, Any})
        if haskey(m, "k_mw_per_degree")
            ref = load("hvdc.json")
            mode = only(filter(x -> x["k_mw_per_degree"] == m["k_mw_per_degree"], ref["inter_area_mode"]))
            mode["tolerance"] = tolerance(m["diff"])
        else
            ref = load(m["file"])
            run = only(filter(r -> r["name"] == m["name"] && r["event"] == m["event"], ref["runs"]))
            run["tolerance"] = Dict(k => tolerance(d) for (k, d) in m["diff"])
        end
    end
    for (f, ref) in files
        for r in ref["runs"]
            @assert haskey(r, "tolerance") "$f: no measurement for $(r["name"]) $(r["event"])"
        end
        open(joinpath(DATA_DIR, f), "w") do io
            JSON3.write(io, ref)
            println(io)
        end
        println("wrote tolerances into ", f)
    end
end

main(ARGS[1])
