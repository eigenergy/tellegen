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

# Component witnesses share one authored input bundle; freeze mapped SI outputs.
for bundle in (isempty(ARGS) ? ["components","bounds","controls","equipment"] : ARGS)
local input = read(joinpath(root, bundle * ".json"), String)
local cases = JSON3.read(input, Dict{String,Any})
local results = Dict{String,Any}()
    # The imported bounds use exactly BMOPFTools' documented unit-test profile.
    # Small authored witnesses use a tighter absolute profile and objective scaling
    # so nearly free tap/Q directions do not stop prematurely at the default mu.
local options = Dict("tol"=>1e-9, "constr_viol_tol"=>1e-9,
        "dual_inf_tol"=>1e-9, "compl_inf_tol"=>1e-9, "bound_relax_factor"=>0.0,
        "acceptable_iter"=>0, "mu_strategy"=>"monotone", "mu_init"=>1e-8,
        "nlp_scaling_method"=>"none", "obj_scaling_factor"=>1000.0)
for name in sort(collect(Base.keys(cases)))
    println("Solving ", name)
    local net = parse_bmopf(JSON3.write(cases[name]); from_string=true)
    local result = bundle == "bounds" ? solve_opf(net) :
        solve_opf(net; s_base=1000.0, solver_options=collect(pairs(options)))
    @assert result["termination_status"] in ("LOCALLY_SOLVED", "OPTIMAL") (name, result["termination_status"])
    results[name] = Dict(k=>result[k] for k in ["objective","bus","line","generator","voltage_source","ibr","transformer","load","termination_status"] if haskey(result,k))
end
local output = Dict("cases"=>results,"provenance"=>Dict(
    "bmopftools_commit"=>commit, "input_sha256"=>bytes2hex(sha256(input)),
    "units"=>"V, A, W, var, currency/hour", "generated_by"=>"scripts/mc_opf_oracle.jl",
    "julia_version"=>string(VERSION), "jump_version"=>string(pkgversion(JuMP)),
    "ipopt_julia_version"=>string(pkgversion(Ipopt)),
    "power_base_va"=>bundle == "bounds" ? 1e6 : 1000.0,
    "solver_profile"=>bundle == "bounds" ? "solve_opf defaults; validation.md unit profile" : "explicit tightly solved authored witnesses",
    "solver_options"=>bundle == "bounds" ? Dict() : options,
    "droop_epsilon_pu"=>2e-3,
    "test_tolerances"=>Dict("objective_currency_per_hour"=>bundle == "bounds" ? 1e-3 : 1e-7,
        "complex_voltage_v"=>bundle == "bounds" ? nothing : bundle == "controls" ? 1e-3 : 2e-4,
        "active_limit_v"=>bundle == "bounds" ? 0.01 : nothing,
        "generator_total_w"=>bundle == "bounds" ? 10.0 : nothing)))
open(joinpath(root,bundle * "-reference.json"),"w") do io
    JSON3.write(io,output);println(io)
end

end
