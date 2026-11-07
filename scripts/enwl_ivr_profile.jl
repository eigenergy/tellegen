# Repeated sequential native / warmed Julia runs; detailed profiles are separate.
# julia --project=/path/to/BMOPFTools.jl/test enwl_ivr_profile.jl cases.json binary output [repeats]
using BMOPFTools, JuMP, Ipopt, JSON3, SHA, LinearAlgebra, Random
length(ARGS) in (3,4) || error("usage: cases.json native-binary output [repeats]")
const OUT = abspath(ARGS[3]); mkpath(OUT)
const BINARY = abspath(ARGS[2])
const OPTIONS = Dict{String,Any}("tol"=>1e-9,"constr_viol_tol"=>1e-9,"bound_relax_factor"=>0.0,"acceptable_iter"=>0,"max_iter"=>500,"print_level"=>0,"linear_solver"=>"mumps")
repo = dirname(dirname(pathof(BMOPFTools)))
strip(read(`git -C $repo rev-parse HEAD`,String)) == "a8b52e069bfd4a7a57434c91bc0470ac03cfdd55" || error("unexpected BMOPFTools revision")
isempty(strip(read(`git -C $repo status --porcelain`,String))) || error("dirty oracle")
BLAS.set_num_threads(1)
paths = JSON3.read(read(ARGS[1],String),Vector{String})
paths = filter(p->startswith(basename(p),"538bus"), paths)
length(paths) == 6 || error("expected six 538-bus cases")
repeats = length(ARGS)==4 ? parse(Int,ARGS[4]) : 5
stem(path)=replace(basename(path),".bmopf.json"=>"")
writejson(path,value)=open(io->JSON3.pretty(io,value),path,"w")
function native(path; profile=false)
    text=read(`$BINARY $path 1000 1 500 $profile`,String)
    row=JSON3.read(last(split(strip(text),'\n')),Dict{String,Any})
    row["result"]["status"]=="accepted" || error(row)
    return row
end
function oracle(path; profile=false, overrides=Dict{String,Any}())
    input=read(path,String)
    trace=Any[]; stages=Dict{String,Float64}()
    options=merge(copy(OPTIONS),overrides)
    if profile
        options["print_level"]=5
        options["print_timing_statistics"]="yes"
        options["output_file"]=joinpath(OUT,stem(path)*"-ipopt.log")
        options["file_print_level"]=5
    end
    t=time_ns(); net=parse_bmopf(input;from_string=true); parsed=time_ns()
    stages["parse"]=(parsed-t)/1e9
    hook = function(ctx)
        JuMP.set_optimize_hook(ctx.model, function(m; kwargs...)
            start=time_ns(); stages["build_and_kcl"]=(start-parsed)/1e9
            JuMP.optimize!(m;ignore_optimize_hook=true,kwargs...)
            stop=time_ns(); stages["optimize_wall"]=(stop-start)/1e9
            stages["optimize_end_ns"]=Float64(stop)
        end)
        if profile
            callback=function(mode, iter, obj, pr, du, mu, dn, reg, ad, ap, ls)
                push!(trace,Dict("mode"=>mode,"iter"=>iter,"objective"=>obj,"inf_pr"=>pr,"inf_du"=>du,"mu"=>mu,"d_norm"=>dn,"regularization"=>reg,"alpha_dual"=>ad,"alpha_primal"=>ap,"ls_trials"=>ls))
                true
            end
            JuMP.set_attribute(ctx.model,Ipopt.CallbackFunction(),callback)
        end
    end
    result=solve_opf(net;s_base=1000.0,solver_options=collect(pairs(options)),model_hook! = hook,verbose=profile)
    stop=time_ns(); stages["postprocess"]=(stop-pop!(stages,"optimize_end_ns"))/1e9
    result["termination_status"]=="LOCALLY_SOLVED" && result["feasible"] || error("reference failed")
    row=Dict("case"=>stem(path),"sha256"=>bytes2hex(sha256(input)),"elapsed_s"=>(stop-t)/1e9,"stages_s"=>stages,"solver_reported_s"=>result["opt_profile"]["solve_time_s"],"iterations"=>result["opt_profile"]["barrier_iterations"],"objective"=>result["objective"],"trace"=>trace)
    return row,result
end
metadata=Dict("julia"=>string(VERSION),"jump"=>string(pkgversion(JuMP)),"ipopt_jl"=>string(pkgversion(Ipopt)),"blas_threads"=>BLAS.get_num_threads(),"julia_threads"=>Threads.nthreads(),"solver_options"=>OPTIONS,"binary_sha256"=>bytes2hex(sha256(read(BINARY))),"thread_env"=>Dict(k=>get(ENV,k,"") for k in ["OMP_NUM_THREADS","OPENBLAS_NUM_THREADS","VECLIB_MAXIMUM_THREADS","RAYON_NUM_THREADS"]),"repeats"=>repeats,"cases"=>paths)
writejson(joinpath(OUT,"metadata.json"),metadata)
for p in paths
    println("Warmup ",stem(p));flush(stdout)
    native(p); oracle(p)
end
rows=Any[]; rng=MersenneTwister(20261006)
for rep in 1:repeats, path in shuffle(rng,paths)
    GC.gc() # outside timing, to avoid debt from preceding solves / JSON handling
    if isodd(rep)
        n=native(path); b,_=oracle(path)
    else
        b,_=oracle(path); n=native(path)
    end
    abs(n["result"]["objective"]-b["objective"])<1e-4 || error("objective mismatch")
    push!(rows,Dict("repeat"=>rep,"case"=>stem(path),"native_s"=>n["elapsed_s"],"native_iterations"=>n["result"]["iterations"],"oracle"=>b))
    writejson(joinpath(OUT,"repeats.json"),rows)
    println("Repeat ",rep," ",stem(path)," native=",n["elapsed_s"]," oracle=",b["elapsed_s"]);flush(stdout)
end
profiles=Any[]
for path in paths
    n=native(path;profile=true); b,result=oracle(path;profile=true)
    writejson(joinpath(OUT,stem(path)*"-native.json"),n)
    writejson(joinpath(OUT,stem(path)*"-oracle.json"),result)
    push!(profiles,Dict("case"=>stem(path),"native_profile"=>n["result"]["profile"],"native_parse_s"=>n["result"]["parse_instance_s"],"native_trace"=>n["iterations"],"oracle"=>b))
    writejson(joinpath(OUT,"profiles.json"),profiles)
end
writejson(joinpath(OUT,"cases.json"),paths)
writejson(joinpath(OUT,"oracle-summary.json"),[merge(p["oracle"],Dict("status"=>"LOCALLY_SOLVED")) for p in profiles])

# A narrow diagnostic: does Ipopt's default gradient scaling explain the
# iteration gap? Keep this separate from the baseline benchmark statistics.
ablations=Any[]
path=only(filter(p->occursin("LN_t12",p),paths))
reference=first(r["oracle"]["objective"] for r in rows if r["case"]==stem(path))
for rep in 1:3
    row,_=oracle(path;overrides=Dict("nlp_scaling_method"=>"none"))
    abs(row["objective"]-reference)<1e-4 || error("scaling ablation objective mismatch")
    push!(ablations,merge(row,Dict("repeat"=>rep,"nlp_scaling_method"=>"none")))
end
writejson(joinpath(OUT,"ipopt-scaling-ablation.json"),ablations)
