# Accessors over two case shapes: the legacy MATPOWER ParsedCase (per-unit
# fields, carries baseMVA) and a PowerIO network (raw MATPOWER units, MW). The
# unit accessors key on baseMVA, so the non-legacy branches assume raw MW values
# and apply no per-unit conversion. Do not pass a normalized (per-unit) PowerIO
# network here: it has no baseMVA, would take the non-legacy branch, and demand,
# generation, and ratings would be under-scaled by baseMVA with no error.
_legacy_case(case) = hasproperty(case, :baseMVA)

case_base_mva(case) =
    _legacy_case(case) ? Float64(case.baseMVA) : Float64(PowerIO.base_mva(case))

case_buses(case) = hasproperty(case, :bus) ? case.bus : PowerIO.buses(case)
case_branches(case) = hasproperty(case, :branch) ? case.branch : PowerIO.branches(case)
case_generators(case) = hasproperty(case, :gen) ? case.gen : PowerIO.generators(case)
case_loads(case) = hasproperty(case, :load) ? case.load : PowerIO.loads(case)

bus_id(b) = hasproperty(b, :bus_i) ? Int(b.bus_i) : Int(b.id)

branch_from(br) = hasproperty(br, :f_bus) ? Int(br.f_bus) : Int(br.from)
branch_to(br) = hasproperty(br, :t_bus) ? Int(br.t_bus) : Int(br.to)
branch_id(br, i) = hasproperty(br, :index) ? Int(br.index) : i
branch_status(br) =
    hasproperty(br, :br_status) ? Int(br.br_status) : (br.in_service ? 1 : 0)
branch_in_service(br) = branch_status(br) == 1
branch_rate_mw(case, br) =
    _legacy_case(case) ? Float64(br.rate_a) * case_base_mva(case) : Float64(br.rate_a)

load_bus(l) = hasproperty(l, :load_bus) ? Int(l.load_bus) : Int(l.bus)
load_in_service(l) = hasproperty(l, :status) ? Int(l.status) == 1 : Bool(l.in_service)
load_p_mw(case, l) =
    _legacy_case(case) ? Float64(l.pd) * case_base_mva(case) : Float64(l.p)

gen_bus(g) = hasproperty(g, :gen_bus) ? Int(g.gen_bus) : Int(g.bus)
gen_in_service(g) =
    hasproperty(g, :gen_status) ? Int(g.gen_status) == 1 : Bool(g.in_service)
gen_pmax_mw(case, g) =
    _legacy_case(case) ? Float64(g.pmax) * case_base_mva(case) : Float64(g.pmax)
