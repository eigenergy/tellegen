# Regenerate the small external IVR oracle. Run with the pinned BMOPFTools
# checkout's test environment; no Julia dependency is needed for cargo test.
# julia --project=/path/to/BMOPFTools.jl/test scripts/mc_opf_oracle.jl
using BMOPFTools, JuMP, Ipopt, JSON3, SHA

repo = dirname(dirname(pathof(BMOPFTools)))
commit = strip(read(`git -C $repo rev-parse HEAD`, String))
expected = "a8b52e069bfd4a7a57434c91bc0470ac03cfdd55"
commit == expected || error("Oracle requires BMOPFTools $expected; found $commit")
isempty(strip(read(`git -C $repo status --porcelain`, String))) || error("BMOPFTools checkout is dirty")
root = joinpath(@__DIR__, "..", "crates", "tellegen", "tests", "data", "mc_opf")
input = read(joinpath(root, "unbalanced.json"), String)
net = parse_bmopf(input; from_string=true)
result = solve_opf(net; s_base=1000.0, solver_options=(
    "tol"=>1e-10, "constr_viol_tol"=>1e-10, "bound_relax_factor"=>0.0))
@assert result["termination_status"] in ("LOCALLY_SOLVED", "OPTIMAL")
keys = ["objective", "bus", "line", "generator", "voltage_source", "termination_status"]
output = Dict(k => result[k] for k in keys)
output["provenance"] = Dict(
    "bmopftools_commit" => commit, "input_sha256" => bytes2hex(sha256(input)),
    "units" => "V, A, W, var, currency/hour", "generated_by" => "scripts/mc_opf_oracle.jl",
    "tolerances" => Dict("objective"=>1e-7, "voltage_v"=>1e-5, "current_a"=>1e-5))
open(joinpath(root, "unbalanced-reference.json"), "w") do io
    JSON3.pretty(io, output)
    println(io)
end
println("BMOPFTools objective: ", result["objective"])
