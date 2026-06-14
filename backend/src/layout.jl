# pglib cases carry no geographic coordinates, so the layout is synthetic: a
# spectral embedding (2nd and 3rd Laplacian eigenvectors, which order buses by
# electrical distance) refined by a Fruchterman-Reingold force pass. The raw
# spectral embedding localizes on networks with long chains: ACTIVSg500
# collapses into a filament with one dense blob. The force pass keeps the
# global spectral ordering while spreading local density, so the network fills
# its service territory the way the real grid does. The API labels these
# coordinates synthetic.

function synthetic_layout(case; bbox=(-94.5, 36.5, -82.5, 43.5))
    ids = sort!([bus_id(b) for b in case_buses(case)])
    idx = Dict(id => i for (i, id) in enumerate(ids))
    n = length(ids)

    L = zeros(n, n)
    edges = Tuple{Int,Int}[]
    seen = Set{Tuple{Int,Int}}()
    for br in case_branches(case)
        branch_in_service(br) || continue
        i = idx[branch_from(br)]
        j = idx[branch_to(br)]
        i == j && continue
        L[i, i] += 1.0
        L[j, j] += 1.0
        L[i, j] -= 1.0
        L[j, i] -= 1.0
        e = minmax(i, j)
        e in seen || (push!(edges, e); push!(seen, e))
    end

    F = eigen(Symmetric(L))
    pos = Matrix{Float64}(undef, n, 2)
    pos[:, 1] = _unit(F.vectors[:, 2])
    pos[:, 2] = _unit(F.vectors[:, 3])
    # Deterministic jitter breaks exact ties (stacked or collinear buses) so
    # the force pass has a direction to separate them along.
    for i in 1:n
        pos[i, 1] += 1e-4 * sin(0.7i)
        pos[i, 2] += 1e-4 * cos(1.3i)
    end
    _force_refine!(pos, edges)

    lon0, lat0, lon1, lat1 = bbox
    xs = _rescale(pos[:, 1], lon0, lon1)
    ys = _rescale(pos[:, 2], lat0, lat1)
    return Dict(ids[i] => (xs[i], ys[i]) for i in 1:n)
end

function _unit(v::AbstractVector)
    lo, hi = extrema(v)
    span = hi - lo
    return span == 0 ? fill(0.5, length(v)) : (v .- lo) ./ span
end

"""
    _force_refine!(pos, edges; iters=250)

Fruchterman-Reingold on the unit square: all pairs repulsion `k^2/d`, spring
attraction `d^2/k` along branches, displacement capped by a linearly cooling
temperature. O(iters * n^2), comfortably inside the boot path at a few
thousand buses. Deterministic.
"""
function _force_refine!(pos::Matrix{Float64}, edges::Vector{Tuple{Int,Int}}; iters=250)
    n = size(pos, 1)
    n > 1 || return pos
    k2 = 1.0 / n            # k^2 with ideal spacing k = sqrt(area/n), area = 1
    k = sqrt(k2)
    disp = zeros(n, 2)
    for it in 1:iters
        fill!(disp, 0.0)
        @inbounds for i in 1:n, j in (i+1):n
            dx = pos[i, 1] - pos[j, 1]
            dy = pos[i, 2] - pos[j, 2]
            f = k2 / (dx * dx + dy * dy + 1e-9)
            disp[i, 1] += dx * f
            disp[i, 2] += dy * f
            disp[j, 1] -= dx * f
            disp[j, 2] -= dy * f
        end
        @inbounds for (i, j) in edges
            dx = pos[i, 1] - pos[j, 1]
            dy = pos[i, 2] - pos[j, 2]
            f = sqrt(dx * dx + dy * dy) / k
            disp[i, 1] -= dx * f
            disp[i, 2] -= dy * f
            disp[j, 1] += dx * f
            disp[j, 2] += dy * f
        end
        t = 0.1 * (1.0 - it / iters) + 1e-3
        @inbounds for i in 1:n
            dx = disp[i, 1]
            dy = disp[i, 2]
            d = sqrt(dx * dx + dy * dy) + 1e-9
            s = min(d, t) / d
            pos[i, 1] += dx * s
            pos[i, 2] += dy * s
        end
    end
    return pos
end

function _rescale(v::AbstractVector, lo::Real, hi::Real)
    vmin, vmax = extrema(v)
    span = vmax - vmin
    span == 0 && return fill((lo + hi) / 2, length(v))
    pad = 0.04 * (hi - lo)
    return (lo + pad) .+ (v .- vmin) ./ span .* ((hi - lo) - 2pad)
end
