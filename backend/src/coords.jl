# Real substation coordinates from a PowerWorld aux export. TAMU complete
# case exports write the substation's latitude/longitude on every bus row
# (the Latitude:1/Longitude:1 columns); powerio keeps those columns in bus
# extras, so one parse of the aux yields the whole map.

"""
    real_coords(aux_path) -> Dict{Int,NTuple{2,Float64}}

Bus id => (lon, lat) read from the aux bus table. Throws when any bus lacks
coordinates: a partially placed network misleads, and the TAMU exports are
complete.
"""
function real_coords(aux_path::AbstractString)
    net = PowerIO.parse_file(aux_path)
    coords = Dict{Int,NTuple{2,Float64}}()
    missed = Int[]
    for b in PowerIO.buses(net)
        lat = _extra_float(b.extras, "Latitude:1", "Latitude")
        lon = _extra_float(b.extras, "Longitude:1", "Longitude")
        if isnothing(lat) || isnothing(lon)
            push!(missed, Int(b.id))
        else
            coords[Int(b.id)] = (lon, lat)
        end
    end
    isempty(missed) || error(
        "$(basename(aux_path)): no coordinates for $(length(missed)) of " *
        "$(length(missed) + length(coords)) buses (first: $(first(missed)))")
    return spread_stacks!(coords)
end

# Aux fields arrive as JSON strings or numbers depending on the column.
function _extra_float(extras, keys...)
    for k in keys
        v = get(extras, k, nothing)
        v isa Number && return Float64(v)
        if v isa AbstractString
            x = tryparse(Float64, strip(v))
            isnothing(x) || return x
        end
    end
    return nothing
end

"""
    spread_stacks!(coords; radius=0.004)

Buses at one substation share its coordinate exactly. Place each co-located
group on a small ring (~400 m) around the substation point so every bus stays
individually hoverable at street zoom; at network zoom the group still reads
as one substation. Deterministic: buses are ordered by id.
"""
function spread_stacks!(coords::Dict{Int,NTuple{2,Float64}}; radius=0.004)
    groups = Dict{NTuple{2,Float64},Vector{Int}}()
    for (id, p) in coords
        push!(get!(groups, p, Int[]), id)
    end
    for (p, ids) in groups
        length(ids) > 1 || continue
        sort!(ids)
        lonscale = max(cosd(p[2]), 0.2)  # keep the ring round on the map
        for (j, id) in enumerate(ids)
            θ = 2π * (j - 1) / length(ids)
            coords[id] = (p[1] + radius * cos(θ) / lonscale, p[2] + radius * sin(θ))
        end
    end
    return coords
end
