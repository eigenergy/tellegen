//! Sparse, row-pivoted, twice-reorthogonalized QR of the active Jacobian.
//! No normal equations, numerical fill dropping, or dense rows are used.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const TOLERANCE: f64 = 1e-9;
const MAX_NONZEROS: usize = 2_000_000;
const MAX_WORK: usize = 100_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfRankOutcome {
    Complete,
    FillLimit,
    WorkLimit,
    Nonfinite,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McOpfRankDiagnostics {
    pub outcome: McOpfRankOutcome,
    pub tolerance: f64,
    pub input_nonzeros: usize,
    /// Peak live coefficients, including the pivot being applied. Indexing
    /// structures use additional storage; this is not an allocator byte count.
    pub peak_nonzeros: usize,
    /// Sparse coefficient visits, including incidence traversal and fill updates.
    pub work: usize,
    pub nonzero_limit: usize,
    pub work_limit: usize,
    pub accepted_pivots: usize,
    pub smallest_accepted_pivot: Option<f64>,
}

pub(super) fn audit(
    rows: Vec<BTreeMap<usize, f64>>,
    n: usize,
) -> (Option<usize>, McOpfRankDiagnostics) {
    audit_with_limits(rows, n, MAX_NONZEROS, MAX_WORK)
}

fn audit_with_limits(
    mut rows: Vec<BTreeMap<usize, f64>>,
    n: usize,
    nonzero_limit: usize,
    work_limit: usize,
) -> (Option<usize>, McOpfRankDiagnostics) {
    use McOpfRankOutcome::*;
    let mut d = McOpfRankDiagnostics {
        outcome: Complete,
        tolerance: TOLERANCE,
        input_nonzeros: rows.iter().map(BTreeMap::len).sum(),
        peak_nonzeros: 0,
        work: 0,
        nonzero_limit,
        work_limit,
        accepted_pivots: 0,
        smallest_accepted_pivot: None,
    };
    let result = (|| -> Result<usize, McOpfRankOutcome> {
        let mut live = d.input_nonzeros;
        d.peak_nonzeros = live;
        if live > nonzero_limit {
            return Err(FillLimit);
        }
        let mut incidence = vec![BTreeSet::new(); n];
        // Nonnegative finite f64 bit patterns sort in numerical order.
        let mut queue = BTreeSet::<(u64, usize)>::new();
        let mut norms = vec![0.; rows.len()];
        for (i, row) in rows.iter_mut().enumerate() {
            charge(&mut d, row.len())?;
            let norm = row.values().fold(0.0_f64, |a, b| a.hypot(*b));
            if !norm.is_finite() || row.keys().any(|&c| c >= n) {
                return Err(Nonfinite);
            }
            if norm > 0. {
                for (&c, v) in row.iter_mut() {
                    *v /= norm;
                    incidence[c].insert(i);
                }
                norms[i] = row.values().fold(0.0_f64, |a, b| a.hypot(*b));
            }
            queue.insert((norms[i].to_bits(), i));
        }
        while let Some((bits, pivot)) = queue.pop_last() {
            let norm = f64::from_bits(bits);
            if norm <= TOLERANCE {
                break;
            }
            let mut basis = std::mem::take(&mut rows[pivot]);
            charge(&mut d, basis.len())?;
            let mut affected = BTreeSet::new();
            for (&c, v) in &mut basis {
                *v /= norm;
                incidence[c].remove(&pivot);
                charge(&mut d, incidence[c].len())?;
                affected.extend(incidence[c].iter().copied());
            }
            d.accepted_pivots += 1;
            d.smallest_accepted_pivot =
                Some(d.smallest_accepted_pivot.map_or(norm, |v| v.min(norm)));
            for i in affected {
                queue.remove(&(norms[i].to_bits(), i));
                let row = &mut rows[i];
                for _ in 0..2 {
                    let (short, long) = if row.len() < basis.len() {
                        (&*row, &basis)
                    } else {
                        (&basis, &*row)
                    };
                    charge(&mut d, short.len())?;
                    let dot: f64 = short
                        .iter()
                        .map(|(c, v)| v * long.get(c).unwrap_or(&0.))
                        .sum();
                    if !dot.is_finite() {
                        return Err(Nonfinite);
                    }
                    if dot == 0. {
                        continue;
                    }
                    charge(&mut d, basis.len())?;
                    for (&c, &q) in &basis {
                        let old = row.get(&c).copied();
                        let value = old.unwrap_or(0.) - dot * q;
                        if !value.is_finite() {
                            return Err(Nonfinite);
                        }
                        if value == 0. {
                            if old.is_some() {
                                row.remove(&c);
                                incidence[c].remove(&i);
                                live -= 1;
                            }
                        } else {
                            if old.is_none() {
                                if live >= nonzero_limit {
                                    return Err(FillLimit);
                                }
                                live += 1;
                                d.peak_nonzeros = d.peak_nonzeros.max(live);
                                incidence[c].insert(i);
                            }
                            row.insert(c, value);
                        }
                    }
                }
                charge(&mut d, row.len())?;
                norms[i] = row.values().fold(0.0_f64, |a, b| a.hypot(*b));
                if !norms[i].is_finite() {
                    return Err(Nonfinite);
                }
                queue.insert((norms[i].to_bits(), i));
            }
            live -= basis.len();
        }
        Ok(d.accepted_pivots)
    })();
    match result {
        Ok(rank) => (Some(rank), d),
        Err(outcome) => {
            d.outcome = outcome;
            (None, d)
        }
    }
}

fn charge(d: &mut McOpfRankDiagnostics, work: usize) -> Result<(), McOpfRankOutcome> {
    if work > d.work_limit.saturating_sub(d.work) {
        return Err(McOpfRankOutcome::WorkLimit);
    }
    d.work += work;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sparse(rows: &[Vec<f64>]) -> Vec<BTreeMap<usize, f64>> {
        rows.iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .filter(|(_, v)| **v != 0.)
                    .map(|(c, v)| (c, *v))
                    .collect()
            })
            .collect()
    }
    #[test]
    fn sparse_qr_agrees_with_dense_pivoted_reference() {
        let mut seed = 17u64;
        for m in [0, 1, 7, 23, 46] {
            for n in [1, 9, 31] {
                let mut rows: Vec<Vec<f64>> = (0..m)
                    .map(|_| {
                        (0..n)
                            .map(|_| {
                                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                                let v = ((seed >> 32) % 11) as f64 - 5.;
                                if v.abs() < 3. {
                                    0.
                                } else {
                                    v
                                }
                            })
                            .collect()
                    })
                    .collect();
                if m > 2 {
                    rows[m - 1] = rows[0].clone();
                }
                let expected = super::super::duals::row_rank(rows.clone());
                let (rank, d) = audit(sparse(&rows), n);
                assert_eq!(d.outcome, McOpfRankOutcome::Complete);
                assert_eq!(rank, Some(expected), "{m} x {n}");
            }
        }
    }
    #[test]
    fn sparse_qualification_exceeds_both_old_size_limits() {
        let rows: Vec<_> = (0..10_000).map(|i| BTreeMap::from([(i, 1.)])).collect();
        let (rank, d) = audit(rows, 10_000);
        assert_eq!(rank, Some(10_000));
        assert_eq!(d.peak_nonzeros, 10_000);
        assert!(d.work < 50_000);
        let (rank, _) = audit(vec![BTreeMap::from([(99_999, 1.)]); 3], 100_000);
        assert_eq!(rank, Some(1));
        // One connected sparse component, not independent dense subproblems.
        let rows = (0..700)
            .map(|i| BTreeMap::from([(i, 1.), (i + 1, -0.2)]))
            .collect();
        let (rank, d) = audit(rows, 701);
        assert_eq!(rank, Some(700), "{d:?}");
        assert!(d.peak_nonzeros < 700 * 701);
    }
    #[test]
    fn numerical_dependence_is_scale_and_permutation_invariant() {
        for epsilon in [1e-12, 1e-6] {
            for scale in [1e-100, 1., 1e100] {
                let mut rows = vec![
                    vec![scale, 0., 0.],
                    vec![1., epsilon, 0.],
                    vec![0., 0., -3.],
                ];
                let expected = if epsilon < TOLERANCE { 2 } else { 3 };
                for _ in 0..3 {
                    assert_eq!(audit(sparse(&rows), 3).0, Some(expected));
                    rows.rotate_left(1);
                    for row in &mut rows {
                        row.rotate_left(1);
                    }
                }
            }
        }
    }
    #[test]
    fn resource_and_nonfinite_refusals_do_not_claim_a_rank() {
        let rows = sparse(&[vec![1., 0., 1.], vec![0., 1., 1.]]);
        for (fill, work, outcome) in [
            (3, 1000, McOpfRankOutcome::FillLimit),
            (4, 1000, McOpfRankOutcome::FillLimit),
            (100, 1, McOpfRankOutcome::WorkLimit),
        ] {
            let (rank, d) = audit_with_limits(rows.clone(), 3, fill, work);
            assert_eq!(rank, None);
            assert_eq!(d.outcome, outcome);
            assert!(d.work <= work);
        }
        assert_eq!(
            audit(vec![BTreeMap::from([(0, f64::NAN)])], 1).1.outcome,
            McOpfRankOutcome::Nonfinite
        );
    }
}
