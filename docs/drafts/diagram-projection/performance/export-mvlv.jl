# Export the complete external MVLV mesh; do not copy DSS or BMOPF data into Tellegen.
# Validate redirects within the common case root using PowerIO's normal include boundary.
# Run with the current BMOPFTools project environment. All output stays external.
using BMOPFTools, SHA
length(ARGS)==1 || error("Usage: julia --project=/path/to/BMOPFTools.jl export-mvlv.jl /path/to/BMOPFDraftData")
repo=realpath(ARGS[1]); root=joinpath(repo,"test","data"); master=joinpath(root,"MVLVmeshed","Master.dss")
inputs=[master]
# The original master lives below the shared MV/LV folders. Put a temporary
# include manifest at their common root so every redirect stays inside the
# parser's case directory; never weaken its include policy or alter source files.
lines=map(split(read(master,String),'\n')) do line
    m=match(r"^\s*redirect\s+([^!]+?)\s*$"i,line)
    m===nothing && return line
    path=realpath(joinpath(dirname(master),strip(m.captures[1])))
    startswith(path,root*"/") || error("Redirect outside external case root")
    push!(inputs,path)
    return "redirect "*relpath(path,root)
end
manifest=tempname(root)*".dss"
try
    write(manifest,join(lines,"\n"))
    net=from_dss(manifest)
    warnings=get(get(net,"_meta",Dict()),"powerio_warnings",[])
    any(w->occursin("INCLUDE_REFUSED",string(w)),warnings) && error("Incomplete include intake")
    length(net["bus"])==3409 || error("Unexpected bus count")
    length(net["switch"])==317 || error("Unexpected switch count")
    length(net["line"])==3098 || error("Unexpected line count")
    sum(length,values(net["transformer"]))==33 || error("Unexpected transformer count")
    net["name"]="MVLVmeshed"
    net["_meta"]["gallery_export"]=Dict("master"=>relpath(master,repo),"input_files"=>[Dict("file"=>relpath(p,repo),"sha256"=>bytes2hex(sha256(read(p)))) for p in unique(inputs)],"switch_states"=>"As emitted by BMOPFTools; gallery scenarios are applied separately.")
    out=joinpath(repo,"output","MVLVmeshed","original","MVLVmeshed.json")
    mkpath(dirname(out));write_bmopf(net,out)
    println("Exported ",length(net["bus"])," buses, ",length(net["switch"])," switches; ",count(s->s["open_switch"],values(net["switch"]))," open in importer output to ",out)
finally
    rm(manifest;force=true)
end
