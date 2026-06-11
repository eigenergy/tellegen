const FRONTEND_BUILD = normpath(joinpath(@__DIR__, "..", "..", "frontend", "build"))

function start(; host="0.0.0.0", port=parse(Int, get(ENV, "TELLEGEN_PORT", "8000")), async=false)
    load_cases!()

    @get "/api/health" function ()
        return (status="ok", cases=sort!(collect(keys(CASES))))
    end

    @get "/api/cases" function ()
        return [(id=e.id, name=e.name, n_bus=e.net.n, n_branch=e.net.m, n_gen=e.net.k)
                for e in (CASES[k] for k in sort!(collect(keys(CASES))))]
    end

    @get "/api/cases/{id}/network" function (req, id::String)
        e = get(CASES, id, nothing)
        isnothing(e) && return _not_found("unknown case $id")
        return _json_response(e.network_json)
    end

    @get "/api/cases/{id}/solution" function (req, id::String)
        e = get(CASES, id, nothing)
        isnothing(e) && return _not_found("unknown case $id")
        return _json_response(e.solution_json)
    end

    # Optional query param d=bus:mw,bus:mw establishes demand = base + deltas
    # before the column is computed, so the gradient is taken at the client's
    # current operating point rather than the boot point.
    @get "/api/cases/{id}/sensitivity/lmp/d/{bus}" function (req, id::String, bus::Int)
        e = get(CASES, id, nothing)
        isnothing(e) && return _not_found("unknown case $id")
        haskey(e.net.id_map.bus_to_idx, bus) || return _not_found("unknown bus $bus")
        deltas = _parse_deltas(get(HTTP.queryparams(req), "d", ""))
        payload = lock(e.lock) do
            establish_demand!(e, deltas)
            sensitivity_payload(e, bus)
        end
        return _json_response(JSON3.write(payload))
    end

    # SSE: solve at demand = base + deltas, streaming interior point
    # iterations as they happen, then the exact solution, then (optionally)
    # the refreshed sensitivity column. GET because EventSource only speaks
    # GET; the perturbation rides in the query string.
    @get "/api/cases/{id}/solve" function (stream::HTTP.Stream, id::String)
        _solve_stream(stream, id)
    end

    isdir(FRONTEND_BUILD) && staticfiles(FRONTEND_BUILD, "/")

    serve(; host, port, async)
end

function _solve_stream(stream::HTTP.Stream, id::String)
    e = get(CASES, id, nothing)
    q = HTTP.queryparams(HTTP.URI(stream.message.target))
    HTTP.setheader(stream, "Content-Type" => "text/event-stream")
    HTTP.setheader(stream, "Cache-Control" => "no-cache")
    if isnothing(e)
        HTTP.setstatus(stream, 404)
        HTTP.startwrite(stream)
        return nothing
    end
    HTTP.startwrite(stream)
    emit(event, payload) =
        write(stream, format_sse_message(JSON3.write(payload); event=event))
    deltas = _parse_deltas(get(q, "d", ""))
    sens_bus = tryparse(Int, get(q, "sens", ""))
    try
        lock(e.lock) do
            target = target_demand(e, deltas)
            elapsed = 0.0
            if target == e.prob.d && e.prob.cache.solution !== nothing
                sol = e.prob.cache.solution
            else
                emit("status", (phase="solving", case=e.id))
                _set_iteration_callback(e.prob) do iter, obj, inf_pr, inf_du
                    emit("iteration", (; iter, objective=obj, inf_pr, inf_du))
                end
                try
                    update_demand!(e.prob, target)
                    elapsed = @elapsed sol = solve!(e.prob)
                finally
                    _clear_iteration_callback(e.prob)
                end
            end
            emit("solution", merge(solution_payload(e.case, e.net, sol),
                (case=e.id, solve_ms=round(1000 * elapsed; digits=1))))
            if !isnothing(sens_bus) && haskey(e.net.id_map.bus_to_idx, sens_bus)
                emit("sensitivity", sensitivity_payload(e, sens_bus))
            end
        end
        emit("done", (ok=true,))
    catch err
        # A vanished client surfaces as an IOError mid-write; anything else
        # (infeasible perturbation, solver failure) goes back on the stream.
        # Named "fail" because EventSource reserves "error" for transport.
        if !(err isa Base.IOError)
            try
                emit("fail", (error=sprint(showerror, err),))
            catch
            end
        end
    finally
        try
            HTTP.closewrite(stream)
        catch
        end
    end
    return nothing
end

"""Stream Ipopt iterates out of the solve. Exceptions in `f` (a closed SSE
stream) return false to Ipopt, which aborts the solve instead of crashing it."""
function _set_iteration_callback(f::Function, prob::DCOPFProblem)
    cb = (alg_mod, iter_count, obj_value, inf_pr, inf_du,
        mu, d_norm, reg_size, alpha_du, alpha_pr, ls_trials) -> begin
        try
            f(Int(iter_count), obj_value, inf_pr, inf_du)
            return true
        catch
            return false
        end
    end
    try
        MOI.set(prob.model, Ipopt.CallbackFunction(), cb)
    catch err
        @warn "iteration callback unavailable" err
    end
    return nothing
end

# CallbackFunction only accepts a Function, so clearing means a no-op.
_clear_iteration_callback(prob::DCOPFProblem) =
    try
        MOI.set(prob.model, Ipopt.CallbackFunction(), (args...) -> true)
    catch
    end

"""Parse `bus:mw,bus:mw` into bus id => MW delta pairs. Malformed entries are
dropped rather than failing the request."""
function _parse_deltas(s::AbstractString)
    deltas = Pair{Int,Float64}[]
    for part in split(s, ','; keepempty=false)
        pieces = split(part, ':')
        length(pieces) == 2 || continue
        bus = tryparse(Int, pieces[1])
        mw = tryparse(Float64, pieces[2])
        (isnothing(bus) || isnothing(mw)) && continue
        push!(deltas, bus => mw)
    end
    return deltas
end

_json_response(body::String) =
    HTTP.Response(200, ["Content-Type" => "application/json"], body)

_not_found(msg::String) =
    HTTP.Response(404, ["Content-Type" => "application/json"], JSON3.write((error=msg,)))
