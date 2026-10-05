# Pinned external-case reference runner. Never writes into the source dataset.
# julia --project=/path/to/BMOPFTools.jl/test scripts/mc_opf_external_oracle.jl cases.json output-directory
using BMOPFTools, JuMP, Ipopt, JSON3, SHA
length(ARGS) == 2 || error("usage: mc_opf_external_oracle.jl cases.json output-directory")
root = abspath(ARGS[2])
mkpath(root)
repo = dirname(dirname(pathof(BMOPFTools)))
commit=strip(read(`git -C $repo rev-parse HEAD`,String))
commit=="a8b52e069bfd4a7a57434c91bc0470ac03cfdd55" || error("unexpected oracle revision")
isempty(strip(read(`git -C $repo status --porcelain`,String))) || error("dirty oracle")
options=Dict("tol"=>1e-9,"constr_viol_tol"=>1e-9,"bound_relax_factor"=>0.0,"acceptable_iter"=>0,"max_iter"=>500,"print_level"=>0)
rows=[]
for path in JSON3.read(read(ARGS[1],String),Vector{String})
    stem=replace(basename(path),".bmopf.json"=>"")
    println("Solving ",stem);flush(stdout)
    input=read(path,String);t=time()
    row=Dict{String,Any}("case"=>stem,"sha256"=>bytes2hex(sha256(input)),"bmopftools_commit"=>commit,"options"=>options,"s_base"=>1000.0,"julia"=>string(VERSION),"jump"=>string(pkgversion(JuMP)),"ipopt"=>string(pkgversion(Ipopt)))
    try
        result=solve_opf(parse_bmopf(input;from_string=true);s_base=1000.0,solver_options=collect(pairs(options)))
        row["status"]=result["termination_status"];row["objective"]=result["objective"];row["elapsed_s"]=time()-t
        # Full result preserves physical variables, solver profile and feasibility data.
        open(joinpath(root,stem*"-oracle.json"),"w") do io; JSON3.write(io,result);end
    catch e
        row["status"]="ERROR";row["error"]=sprint(showerror,e);row["elapsed_s"]=time()-t
    end
    push!(rows,row)
    open(joinpath(root,"oracle-summary.json"),"w") do io;JSON3.pretty(io,rows);end
    println(JSON3.write(row));flush(stdout)
end
