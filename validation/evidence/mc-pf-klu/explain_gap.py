"""Locate and explain the Austin P3U solver disagreement.

Arguments: the directory written by export_gap_case.py and, optionally, the
Rust test executable (to include faer's own answer). Prints a JSON report.

"Exact" below means the float64 vector reached by iterative refinement whose
residuals are summed without rounding error (error-free products, math.fsum).
It is the solution of the exported float64 matrix, not of the circuit model.
"""

import json
import math
import sys
import tempfile
from pathlib import Path

import numpy as np
import scipy.sparse as sp
import scipy.sparse.linalg as sla
from scipy.sparse.csgraph import connected_components

DRAWS = 5
ANTIFLOAT_FACTORS = ["0.1", "10", "100"]


def two_product(a, b):
    def split(v):
        c = 134217729.0 * v
        high = c - (c - v)
        return high, v - high

    p = a * b
    ah, al = split(a)
    bh, bl = split(b)
    return p, ((ah * bh - p) + ah * bl + al * bh) + al * bl


def residual(matrix, x, rhs):
    """rhs - matrix @ x, each component correctly rounded (matrix in CSR)."""
    xr, xi = x.real[matrix.indices], x.imag[matrix.indices]
    ar, ai = matrix.data.real, matrix.data.imag
    real = -np.stack([*two_product(ar, xr), *two_product(-ai, xi)], 1).reshape(-1)
    imag = -np.stack([*two_product(ar, xi), *two_product(ai, xr)], 1).reshape(-1)
    real, imag, start = real.tolist(), imag.tolist(), (4 * matrix.indptr).tolist()
    br, bi = rhs.real.tolist(), rhs.imag.tolist()
    return np.array(
        [
            complex(
                math.fsum(real[start[i] : start[i + 1]] + [br[i]]),
                math.fsum(imag[start[i] : start[i + 1]] + [bi[i]]),
            )
            for i in range(len(br))
        ]
    )


def exact_solution(matrix, rhs):
    factor = sla.splu(matrix.tocsc())
    x = factor.solve(rhs)
    for _ in range(12):
        step = factor.solve(residual(matrix, x, rhs))
        x = x + step
        if np.max(abs(step)) < 1e-9:
            return x, float(np.max(abs(step)))
    raise AssertionError("refinement did not converge")


def exact_sum(values):
    values = np.asarray(values)
    return complex(math.fsum(values.real), math.fsum(values.imag))


def split_difference(difference, sections, pairs):
    """Per-section common shift, what is left after removing it, and the rest."""
    outside = np.ones(len(difference), bool)
    rows = []
    for nodes, (left, right) in zip(sections, pairs):
        outside[nodes] = False
        part = difference[nodes]
        common = part.mean()
        rest = part - common
        rows.append(
            {
                "max_difference_v": float(np.max(abs(part))),
                "common_shift_v": [common.real, common.imag],
                "common_shift_magnitude_v": abs(common),
                "common_share_of_squared_norm": float(
                    len(part) * abs(common) ** 2 / np.sum(abs(part) ** 2)
                ),
                "after_removing_common_max_v": float(np.max(abs(rest))),
                "after_removing_common_rms_v": float(np.sqrt(np.mean(abs(rest) ** 2))),
                "max_between_coupled_nodes_v": float(
                    np.max(abs(difference[left] - difference[right]))
                ),
            }
        )
    return {
        "sections": rows,
        "max_outside_sections_v": float(np.max(abs(difference[outside]))),
        "nodes_outside_sections_above_1e-3_v": int(
            np.count_nonzero(abs(difference[outside]) > 1e-3)
        ),
    }


def perturb(matrix, rng, mask):
    """Move the real and imaginary part of each chosen entry by -1, 0 or +1 ulp."""
    data = matrix.data.copy()
    for part in (data.real, data.imag):
        move = rng.integers(-1, 2, len(part)) * mask
        part[:] = np.where(
            move == 0, part, np.nextafter(part, np.where(move > 0, np.inf, -np.inf))
        )
    return sp.csr_matrix((data, matrix.indices, matrix.indptr), shape=matrix.shape)


def run():
    directory = Path(sys.argv[1])
    executable = sys.argv[2] if len(sys.argv) > 2 else None
    matrix = sp.load_npz(directory / "Y.npz").tocsr()
    matrix.sort_indices()
    vectors = np.load(directory / "vectors.npz")
    klu, rhs = vectors["klu"], vectors["rhs"]
    model = json.loads((directory / "elements.json").read_text())
    names, elements = model["nodes"], model["elements"]
    omega = 2 * math.pi * model["frequency_hz"]
    n = len(klu)
    report = {"dimension": n, "nonzeros": int(matrix.nnz)}

    # The 69 kV level and its connected sections.
    level = sorted(
        {
            node
            for e in elements
            for k, node in enumerate(e["nodes"])
            if node >= 0 and e["at_69kv"][k // e["conductors"]]
        }
    )
    level = np.array(level)
    count, label = connected_components(abs(matrix[level][:, level]) > 0, False)
    sections = [level[label == k] for k in range(count)]
    pairs = []
    for nodes in sections:
        block = sp.triu(matrix[nodes][:, nodes], 1).tocoo()
        pairs.append((nodes[block.row], nodes[block.col]))

    # Other solvers and the exact solution of the exported matrix.
    solutions = {"klu": klu}
    for ordering in ["COLAMD", "MMD_AT_PLUS_A"]:
        solutions[f"superlu_{ordering.lower()}"] = sla.splu(
            matrix.tocsc(), permc_spec=ordering
        ).solve(rhs)
    exact, last_step = exact_solution(matrix, rhs)
    report["exact_refinement_last_correction_v"] = last_step
    if executable:
        import compare

        coo = matrix.tocoo()
        exported = {
            "terminals": [name.rsplit(".", 1) for name in names],
            "entries": [
                (int(r), int(c), [v.real, v.imag])
                for r, c, v in zip(coo.row, coo.col, coo.data)
            ],
            "rhs": [[v.real, v.imag] for v in rhs],
        }
        with tempfile.TemporaryDirectory() as scratch:
            solved = compare.faer(executable, scratch, matrix=exported)["faer"]
        solutions["faer"] = compare.values(solved["solution"])
        report["faer_high_precision_refined_vs_exact_v"] = float(
            np.max(
                abs(compare.values(solved["high_precision_refined_solution"]) - exact)
            )
        )
    solutions["exact"] = exact

    # Ground path of each section, from the matrix and from the element data.
    model_shunt = np.zeros(n, complex)
    formula_check = 0.0
    largest = np.zeros(n)
    for e in elements:
        size = len(e["nodes"])
        primitive = np.array([complex(*v) for v in e["yprim"]]).reshape(size, size)
        nodes = np.array(e["nodes"])
        conductors = e["conductors"]
        for terminal, at_level in enumerate(e["at_69kv"]):
            if not at_level:
                continue
            block = slice(terminal * conductors, (terminal + 1) * conductors)
            here = [
                k
                for k in range(size)
                if nodes[k] >= 0 and e["at_69kv"][k // conductors]
            ]
            from_primitive = {
                k: exact_sum(primitive[k, here])
                for k in range(block.start, block.stop)
                if nodes[k] >= 0
            }
            for k in from_primitive:
                largest[nodes[k]] = max(largest[nodes[k]], np.max(abs(primitive[k])))
            if "length" in e:
                capacitance = np.array(e["capacitance_nf_per_length"]).sum(1)
                for phase, k in enumerate(range(block.start, block.stop)):
                    value = 0.5j * omega * 1e-9 * e["length"] * capacitance[phase]
                    model_shunt[nodes[k]] += value
                    if np.max(abs(primitive)) < 1e4:
                        formula_check = max(
                            formula_check, abs(value - from_primitive[k]) / abs(value)
                        )
            else:
                for k, value in from_primitive.items():
                    model_shunt[nodes[k]] += value
    report["line_shunt_formula_max_relative_mismatch"] = formula_check

    bounding = [e for e in elements if "windings" in e]
    lines = [e for e in elements if "length" in e]
    described = []
    for nodes in sections:
        member = np.zeros(n, bool)
        member[nodes] = True
        indicator = member.astype(float)
        product = matrix @ indicator
        block = matrix[nodes][:, nodes]
        shunt = np.array(
            [
                exact_sum(block.data[block.indptr[k] : block.indptr[k + 1]])
                for k in range(len(nodes))
            ]
        )
        total, model_total = exact_sum(shunt), exact_sum(model_shunt[nodes])

        def inside(e):
            return any(member[k] for k in e["nodes"] if k >= 0)

        windings = {}
        antifloat = 0j
        for e in filter(inside, bounding):
            key = " / ".join(
                f"{w['connection']} {w['kv']:g} kV {w['kva'] / 1000:g} MVA"
                for w in e["windings"]
            )
            windings[key] = windings.get(key, 0) + 1
            size = len(e["nodes"])
            primitive = np.array([complex(*v) for v in e["yprim"]]).reshape(size, size)
            here = [
                k for k in range(size) if e["nodes"][k] >= 0 and member[e["nodes"][k]]
            ]
            antifloat += exact_sum(primitive[np.ix_(here, here)].ravel())
        section_lines = list(filter(inside, lines))
        fictitious = shunt - model_shunt[nodes]
        by_bus = {}
        for k, value in zip(nodes, shunt):
            by_bus.setdefault(names[k].rsplit(".", 1)[0], []).append(value)
        by_bus = {bus: np.array(group) for bus, group in by_bus.items()}
        row = {
            "nodes": len(nodes),
            "buses": len({names[k].rsplit(".", 1)[0] for k in nodes}),
            "klu_voltage_magnitude_range_v": [
                float(abs(klu[nodes]).min()),
                float(abs(klu[nodes]).max()),
            ],
            "rhs_max_a": float(np.max(abs(rhs[nodes]))),
            "coupling_of_common_shift_to_other_nodes_s": float(
                np.max(abs(product[~member]))
            ),
            "transformers": windings,
            "antifloat_ppm": sorted(
                {e["ppm_antifloat"] for e in filter(inside, bounding)}
            ),
            "antifloat_admittance_s": [antifloat.real, antifloat.imag],
            "lines": len(section_lines),
            "line_length_km": sum(e["length"] for e in section_lines),
            "matrix_admittance_to_ground_s": [total.real, total.imag],
            "model_admittance_to_ground_s": [model_total.real, model_total.imag],
            "largest_matrix_entry_s": float(np.max(abs(block.data))),
            "ulp_of_largest_entry_s": float(np.spacing(np.max(abs(block.data.real)))),
            "nodes_touching_entries_above_1e7_s": int(
                np.count_nonzero(abs(block.diagonal()) > 1e7)
            ),
            "largest_node_shunt_difference_matrix_vs_model_s": float(
                np.max(abs(fictitious))
            ),
            "max_spread_of_node_shunt_across_phases_of_a_bus_s": max(
                float(np.max(abs(group - group[0]))) for group in by_bus.values()
            ),
            "common_shift_from_one_ulp_of_largest_entry_v": float(
                np.spacing(np.max(abs(block.data.real)))
                * np.max(abs(klu[nodes]))
                / abs(total)
            ),
            "solutions": {},
        }
        for name, x in solutions.items():
            common = x[nodes].mean()
            to_ground = exact_sum(shunt * x[nodes])
            row["solutions"][name] = {
                "common_voltage_v": [common.real, common.imag],
                "common_voltage_magnitude_v": abs(common),
                "net_current_to_ground_a": abs(to_ground),
                "net_current_over_admittance_v": abs(to_ground / total),
            }
        # First-order effect of the rounding already present in the stored
        # entries: the shift that makes the element-data shunts carry no net current.
        shift = -exact_sum(model_shunt[nodes] * exact[nodes]) / model_total
        row["common_shift_from_rounding_of_stored_entries_v"] = abs(shift)
        described.append(row)
    report["sections"] = described
    report["nodes_at_69kv"] = len(level)

    report["differences"] = {
        f"{a}_minus_{b}": split_difference(solutions[a] - solutions[b], sections, pairs)
        for a, b in [
            ("klu", "exact"),
            ("superlu_colamd", "exact"),
            ("superlu_mmd_at_plus_a", "exact"),
            ("superlu_colamd", "klu"),
        ]
        + ([("faer", "klu"), ("faer", "exact")] if executable else [])
    }

    # Element currents and powers at the 69 kV level, KLU against the others.
    def flows(x):
        currents, powers = [], []
        for e in elements:
            size = len(e["nodes"])
            primitive = np.array([complex(*v) for v in e["yprim"]]).reshape(size, size)
            nodes = np.array(e["nodes"])
            v = np.where(nodes >= 0, x[nodes], 0)
            current = primitive @ v
            currents.append(current)
            powers.append((v * current.conj()).reshape(-1, e["conductors"]).sum(1))
        return np.concatenate(currents), np.concatenate(powers)

    reference_current, reference_power = flows(klu)
    report["element_flows_vs_klu"] = {
        "elements": len(elements),
        "largest_conductor_current_a": float(np.max(abs(reference_current))),
        "largest_terminal_power_va": float(np.max(abs(reference_power))),
    }
    for name, x in solutions.items():
        if name != "klu":
            current, power = flows(x)
            report["element_flows_vs_klu"][name] = {
                "max_conductor_current_difference_a": float(
                    np.max(abs(current - reference_current))
                ),
                "max_terminal_power_difference_va": float(
                    np.max(abs(power - reference_power))
                ),
            }

    # One-ulp changes of the matrix entries, each solved exactly.
    rng = np.random.default_rng(153)
    large = abs(matrix.data) > 1e7
    in_level = np.zeros(n, bool)
    in_level[level] = True
    rows_of = np.repeat(np.arange(n), np.diff(matrix.indptr))
    masks = {
        "all_entries": np.ones(matrix.nnz, bool),
        "only_69kv_entries_above_1e7_s": large & in_level[rows_of],
        "all_other_entries": ~(large & in_level[rows_of]),
    }
    report["one_ulp_perturbations"] = {}
    for name, mask in masks.items():
        shifts, rests, outside = [], [], []
        for _ in range(DRAWS):
            moved, _ = exact_solution(perturb(matrix, rng, mask), rhs)
            parts = split_difference(moved - exact, sections, pairs)
            shifts.append([s["common_shift_magnitude_v"] for s in parts["sections"]])
            rests.append(
                max(s["after_removing_common_max_v"] for s in parts["sections"])
            )
            outside.append(parts["max_outside_sections_v"])
        report["one_ulp_perturbations"][name] = {
            "entries_perturbed": int(np.count_nonzero(mask)),
            "common_shift_magnitude_v_per_draw_and_section": shifts,
            "max_after_removing_common_v": max(rests),
            "max_outside_sections_v": max(outside),
        }

    # Smallest singular values. Pieces with no connection to the source are
    # separate diagonal blocks, so they are handled apart from the main piece.
    count, label = connected_components(abs(matrix) > 0, False)
    sizes = np.bincount(label)
    main = np.where(label == np.argmax(sizes))[0]
    rest = np.where(label != np.argmax(sizes))[0]
    norm = float(sla.svds(matrix.tocsc(), k=1, return_singular_vectors=False)[0])
    detached = np.linalg.svd(matrix[rest][:, rest].toarray(), compute_uv=False)
    sub = matrix[main][:, main].tocsc()
    factor = sla.splu(sub)
    inverse = sla.LinearOperator(
        sub.shape,
        matvec=lambda v: factor.solve(factor.solve(v.astype(complex), trans="H")),
        dtype=complex,
    )
    values, vectors = sla.eigsh(inverse, k=6, which="LM")
    order = np.argsort(-values)
    singular, vectors = 1 / np.sqrt(values[order]), vectors[:, order]
    position = np.full(n, -1)
    position[main] = np.arange(len(main))
    report["singular_values"] = {
        "largest_s": norm,
        "pieces_not_connected_to_source": int(count - 1),
        "nodes_in_those_pieces": len(rest),
        "max_klu_voltage_in_those_pieces_v": float(np.max(abs(klu[rest]))),
        "max_rhs_in_those_pieces_a": float(np.max(abs(rhs[rest]))),
        "max_solver_difference_in_those_pieces_v": float(
            max(np.max(abs(x[rest] - klu[rest])) for x in solutions.values())
        ),
        "smallest_in_those_pieces_s": float(detached.min()),
        "ratio_largest_to_smallest_overall": norm / float(detached.min()),
        "smallest_six_in_main_piece_s": singular.tolist(),
        "ratio_largest_to_smallest_main_piece": norm / float(singular[0]),
        "sections": [],
    }
    for nodes in sections:
        indicator = np.zeros(len(main))
        indicator[position[nodes]] = 1 / math.sqrt(len(nodes))
        quotient = indicator @ (sub @ indicator)
        report["singular_values"]["sections"].append(
            {
                "indicator_rayleigh_quotient_s": abs(quotient),
                "indicator_share_in_two_smallest_singular_vectors": float(
                    np.sum(abs(vectors[:, :2].conj().T @ indicator) ** 2)
                ),
            }
        )

    # ppm_antifloat scaled in OpenDSS on the transformers with a 69 kV winding.
    report["antifloat_scaling"] = {}
    for factor_name in ANTIFLOAT_FACTORS:
        changed = sp.load_npz(directory / f"Y-antifloat-x{factor_name}.npz").tocsr()
        changed.sort_indices()
        changed_vectors = np.load(directory / f"vectors-antifloat-x{factor_name}.npz")
        changed_exact, _ = exact_solution(changed, changed_vectors["rhs"])
        entry = {
            "max_rhs_change_a": float(np.max(abs(changed_vectors["rhs"] - rhs))),
            "matrix_admittance_to_ground_s": [],
            "klu": split_difference(changed_vectors["klu"] - klu, sections, pairs),
            "exact": split_difference(changed_exact - exact, sections, pairs),
            "klu_common_voltage_magnitude_v": [],
            "exact_common_voltage_magnitude_v": [],
        }
        for nodes in sections:
            total = exact_sum(changed[nodes][:, nodes].data)
            entry["matrix_admittance_to_ground_s"].append([total.real, total.imag])
            entry["klu_common_voltage_magnitude_v"].append(
                abs(changed_vectors["klu"][nodes].mean())
            )
            entry["exact_common_voltage_magnitude_v"].append(
                abs(changed_exact[nodes].mean())
            )
        report["antifloat_scaling"][f"x{factor_name}"] = entry

    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    run()
