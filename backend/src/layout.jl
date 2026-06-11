# pglib cases carry no geographic coordinates. The 2nd and 3rd Laplacian
# eigenvectors give a planar embedding ordered by electrical distance, which
# reads like a one line diagram when scaled into a bbox on the map. The API
# labels these coordinates synthetic.

function spectral_layout(case::ParsedCase; bbox=(-94.5, 36.5, -82.5, 43.5))
    ids = sort!([b.bus_i for b in case.bus])
    idx = Dict(id => i for (i, id) in enumerate(ids))
    n = length(ids)
    L = zeros(n, n)
    for br in case.branch
        br.br_status == 1 || continue
        i = idx[br.f_bus]
        j = idx[br.t_bus]
        i == j && continue
        L[i, i] += 1.0
        L[j, j] += 1.0
        L[i, j] -= 1.0
        L[j, i] -= 1.0
    end
    F = eigen(Symmetric(L))
    lon0, lat0, lon1, lat1 = bbox
    xs = _rescale(F.vectors[:, 2], lon0, lon1)
    ys = _rescale(F.vectors[:, 3], lat0, lat1)
    return Dict(ids[i] => (xs[i], ys[i]) for i in 1:n)
end

function _rescale(v::AbstractVector, lo::Real, hi::Real)
    vmin, vmax = extrema(v)
    span = vmax - vmin
    span == 0 && return fill((lo + hi) / 2, length(v))
    pad = 0.04 * (hi - lo)
    return (lo + pad) .+ (v .- vmin) ./ span .* ((hi - lo) - 2pad)
end
