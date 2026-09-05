//! Create an explicitly labelled lower-convex-cost scenario for a Study example.

use std::io::Write;

fn lower_hull(points: &[[f64; 2]]) -> Result<Vec<[f64; 2]>, String> {
    if points.len() < 2
        || points.iter().flatten().any(|x| !x.is_finite())
        || points.windows(2).any(|p| p[0][0] >= p[1][0])
    {
        return Err("piecewise costs require increasing powers and finite points".into());
    }
    let slope = |a: [f64; 2], b: [f64; 2]| (b[1] - a[1]) / (b[0] - a[0]);
    let mut hull: Vec<[f64; 2]> = Vec::new();
    for &point in points {
        while hull.len() >= 2
            && slope(hull[hull.len() - 2], hull[hull.len() - 1])
                > slope(hull[hull.len() - 1], point)
        {
            hull.pop();
        }
        hull.push(point);
    }
    Ok(hull)
}

fn write_new(path: &str, text: &str) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err(
            "usage: convex_cost_scenario INPUT.pio.json OUTPUT.pio.json REPORT.json".into(),
        );
    }
    let source = std::fs::read_to_string(&args[0]).map_err(|e| e.to_string())?;
    let module = tellegen::ir::balanced_module(tellegen::ir::deserialize_module(&source)?)?;
    let mut changes = Vec::new();
    let module = module.try_map_value(|mut network| {
        for (row, generator) in network.generators_mut().iter_mut().enumerate() {
            let Some(cost) = generator.cost.as_mut().filter(|cost| cost.model == 1) else {
                continue;
            };
            if cost.coeffs.len() != 2 * cost.ncost {
                return Err("piecewise cost point count disagrees with ncost".to_string());
            }
            let points = cost.coeffs.chunks_exact(2).map(|p| [p[0], p[1]]).collect::<Vec<_>>();
            let hull = lower_hull(&points)?;
            if hull.len() == points.len() {
                continue;
            }
            let max_reduction = points.iter().map(|p| {
                let segment = hull.windows(2).find(|s| s[0][0] <= p[0] && p[0] <= s[1][0]).unwrap();
                let t = (p[0] - segment[0][0]) / (segment[1][0] - segment[0][0]);
                p[1] - (segment[0][1] + t * (segment[1][1] - segment[0][1]))
            }).fold(0.0, f64::max);
            changes.push(serde_json::json!({ "generator_row_zero_based": row, "uid": generator.uid,
                "before": points, "after": hull, "maximum_cost_reduction_per_hour": max_reduction }));
            cost.ncost = hull.len();
            cost.coeffs = hull.into_iter().flatten().collect();
        }
        Ok(network)
    })?;
    let output = tellegen::ir::serialize_module(&module)?;
    let report = serde_json::json!({ "schema": "tellegen.cost-scenario/1",
        "interpretation": "Replace each nonconvex piecewise cost with its lower convex envelope over the stated points. This changes the inner economic model and does not solve the original nonconvex OPF.",
        "source_sha256": tellegen::document::content_id(source.as_bytes()),
        "scenario_sha256": tellegen::document::content_id(output.as_bytes()),
        "changed_generators": changes.len(), "changes": changes });
    write_new(&args[1], &output)?;
    write_new(
        &args[2],
        &serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
    )
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn envelope_retains_convex_points_and_bounds_the_declared_cost() {
        assert_eq!(
            lower_hull(&[[0., 0.], [1., 1.], [2., 4.]]).unwrap(),
            vec![[0., 0.], [1., 1.], [2., 4.]]
        );
        assert_eq!(
            lower_hull(&[[0., 0.], [1., 3.], [2., 4.], [3., 7.]]).unwrap(),
            vec![[0., 0.], [2., 4.], [3., 7.]]
        );
        assert!(lower_hull(&[[1., 0.], [1., 2.]]).is_err());
    }
}
