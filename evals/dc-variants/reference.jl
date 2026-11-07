# Independent validation-only formulations. Built-in DCP and DCMP are unmodified.
using PowerModels, Ipopt, JSON
import JuMP
const PM = PowerModels
PM.silence()
mutable struct ReferenceDC{C} <: PM.AbstractDCPModel
    PM.@pm_fields
end
function coefficient(::ReferenceDC{C}, g, b, tm, tr, ti) where C
    C == :legacy && return (-b, atan(ti, tr))
    C == :hybrid && return (-b/tm, atan(ti, tr))
    C == :reactance && return (-(g*g+b*b)/b, 0.0)
    error("unknown reference")
end
function PM.constraint_ohms_yt_from(pm::ReferenceDC, n::Int, f, t, fi, ti, g, b, gf, bf, tr, im, tm)
    k, shift = coefficient(pm, g, b, tm, tr, im)
    JuMP.@constraint(pm.model, PM.var(pm,n,:p,fi) == k*(PM.var(pm,n,:va,f)-PM.var(pm,n,:va,t)-shift))
end
function PM.constraint_ohms_yt_to(pm::ReferenceDC, n::Int, f, t, fi, ti, g, b, gt, bt, tr, im, tm)
    JuMP.@constraint(pm.model, PM.var(pm,n,:p,ti) == -PM.var(pm,n,:p,fi))
end
function PM.expression_branch_power_ohms_yt_from(pm::ReferenceDC, n::Int, f, t, fi, ti, g, b, gf, bf, tr, im, tm)
    k, shift = coefficient(pm, g, b, tm, tr, im)
    PM.var(pm,n,:p)[fi] = k*(PM.var(pm,n,:va,f)-PM.var(pm,n,:va,t)-shift)
end
function PM.expression_branch_power_ohms_yt_to(pm::ReferenceDC, n::Int, f, t, fi, ti, g, b, gt, bt, tr, im, tm)
    k, shift = coefficient(pm, g, b, tm, tr, im)
    PM.var(pm,n,:p)[ti] = -k*(PM.var(pm,n,:va,f)-PM.var(pm,n,:va,t)-shift)
end
optimizer = JuMP.optimizer_with_attributes(Ipopt.Optimizer, "print_level"=>0, "tol"=>1e-10, "bound_relax_factor"=>0.0, "max_iter"=>1500, "max_cpu_time"=>300.0)
function pf(data, model)
    PM.solve_model(data, model, optimizer, pm -> begin
        PM.build_pf(pm)
        n = PM.nw_id_default
        PM.sol_component_value_edge(pm,n,:branch,:pf,:pt,PM.ref(pm,n,:arcs_from),PM.ref(pm,n,:arcs_to),PM.var(pm,n,:p))
        if model == PM.ACPPowerModel
            PM.sol_component_value_edge(pm,n,:branch,:qf,:qt,PM.ref(pm,n,:arcs_from),PM.ref(pm,n,:arcs_to),PM.var(pm,n,:q))
        end
    end)
end
case, tellegen_path, output = ARGS
source = PM.parse_file(case)
# Explicit comparison policy: intersect parsed angle windows with +/-60
# degrees. The three recorded fixtures already use this finite window.
for branch in values(source["branch"])
    branch["angmin"] = max(branch["angmin"], -pi/3)
    branch["angmax"] = min(branch["angmax"], pi/3)
end
models = Dict("series_susceptance_no_phase_shift"=>PM.DCPPowerModel,
    "tap_adjusted_reactance"=>PM.DCMPPowerModel,
    "series_susceptance"=>ReferenceDC{:legacy},
    "tap_adjusted_series_susceptance"=>ReferenceDC{:hybrid},
    "reactance_only"=>ReferenceDC{:reactance})
function attempt(fn)
    try
        fn()
    catch e
        Dict("error"=>sprint(showerror, e))
    end
end
results = Dict{String,Any}()
for (name, model) in models
    results[name] = Dict("dcpf"=>attempt(()->pf(deepcopy(source),model)),
        "dcopf"=>attempt(()->PM.solve_opf(deepcopy(source),model,optimizer; setting=Dict("output"=>Dict("duals"=>true)))))
end
# Fixed PV/reference voltage setpoints. Q limits are deliberately not enforced
# by PF; all violations are measured afterwards. Reference generation absorbs losses.
results["ac_pf"] = attempt(()->pf(deepcopy(source),PM.ACPPowerModel))
if length(source["bus"]) <= 300
    results["ac_opf_local"] = attempt(()->PM.solve_opf(deepcopy(source),PM.ACPPowerModel,optimizer))
end
replay = Dict{String,Any}()
for (name, solves) in JSON.parsefile(tellegen_path)
    dispatch = get(solves["dcopf"], "dispatch", nothing)
    dispatch === nothing && continue
    data = deepcopy(source)
    for gen in dispatch
        data["gen"][string(gen["gen"])]["pg"] = gen["pg"]/data["baseMVA"]
    end
    replay[name] = attempt(()->pf(data, PM.ACPPowerModel))
end
results["ac_replay"] = replay
results["data"] = source
results["versions"] = Dict("julia"=>string(VERSION), "PowerModels"=>string(Base.pkgversion(PM)), "Ipopt"=>string(Base.pkgversion(Ipopt)))
open(output,"w") do io
    JSON.print(io,results,2)
end
