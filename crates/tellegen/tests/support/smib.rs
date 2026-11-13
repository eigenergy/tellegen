// Single-machine infinite-bus (SMIB) cases built from the WECC 240 case that
// ships in `tests/data/rms_examples/wecc240`. Included by the tests that use them.
//
// One case per combination of dynamic models WECC 240 uses: the first in-service
// unit that carries the combination is placed on the one-machine infinite-bus
// system of the PowerSimulationsDynamics.jl PSS/E benchmarks: an infinite bus
// 101 (GENCLS with H = 0) and the unit at bus 102, joined by two parallel lines.
// The unit keeps its DYR records (renumbered to bus 102, machine id 1), its
// dispatch PG, its MBASE, ZSORCE and voltage setpoint VS. The lines are scaled
// with the unit's MBASE (x = 0.1 pu on 100 MVA in the template, so
// x = 0.1 * 100 / MBASE per circuit here), so every unit sees the same
// electrical distance whatever its size. The infinite bus holds 1.0 pu.
//
// This is the construction of the SMIB validation against
// PowerSimulationsDynamics.jl (`docs/src/rms-validation.md`); the numbers it
// writes are rounded as there, so the cases are the same to the last digit.

/// A SMIB case: its name, the sorted model names, the source unit, the raw
/// and dyr text, and the per-circuit line reactance.
#[allow(dead_code)]
pub struct SmibCase {
    pub id: String,
    pub models: Vec<String>,
    pub unit: (usize, String),
    pub raw: String,
    pub dyr: String,
    pub x_line: f64,
}

#[allow(dead_code)]
fn wecc240_text(file: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/rms_examples/wecc240")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

const SMIB_TAIL: &str = " 0 /End of Branch data, Begin Transformer data
 0 /End of Transformer data, Begin Area interchange data
 0 /End of Area interchange data, Begin Two-terminal dc line data
 0 /End of Two-terminal dc line data, Begin VSC dc line data
 0 /End of VSC dc line data, Begin Impedance correction table data
 0 /End of Impedance correction table data, Begin Multi-terminal dc line data
 0 /End of Multi-terminal dc line data, Begin Multi-section line data
 0 /End of Multi-section line data, Begin Zone data
 0 /End of Zone data, Begin Inter-area transfer data
 0 /End of Inter-area transfer data, Begin Owner data
 0 /End of Owner data, Begin FACTS device data
 0 /End of FACTS device data, Begin Switched shunt data
 0 /End of Switched shunt data, Begin GNE device data
 0 /End of GNE device data
Q
";

/// `{:.5E}` as Python writes it (`1.00000E-05`).
#[allow(dead_code)]
fn sci5(v: f64) -> String {
    let s = format!("{v:.5e}");
    let (m, e) = s.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    format!("{m}E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// Every SMIB case of WECC 240, in the order their units first appear in its
/// DYR file.
#[allow(dead_code)]
pub fn wecc240_smib_cases() -> Vec<SmibCase> {
    let raw = wecc240_text("case.raw");
    let lines: Vec<&str> = raw.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.contains("Begin Generator data"))
        .unwrap()
        + 1;
    let end = lines
        .iter()
        .position(|l| l.contains("End of Generator data"))
        .unwrap();
    let mut gens: std::collections::BTreeMap<(String, String), Vec<String>> =
        std::collections::BTreeMap::new();
    for line in &lines[start..end] {
        let f: Vec<String> = line.split(',').map(|x| x.trim().to_owned()).collect();
        let id = f[1].trim_matches('\'').trim().to_owned();
        gens.insert((f[0].clone(), id), f);
    }
    // DYR records grouped by unit, units in order of first appearance.
    let dyr = wecc240_text("case.dyr");
    let mut order: Vec<(String, String)> = Vec::new();
    let mut records: std::collections::BTreeMap<(String, String), Vec<String>> =
        std::collections::BTreeMap::new();
    for rec in dyr.split('/').map(str::trim).filter(|r| !r.is_empty()) {
        let tok: Vec<String> = rec.replace(',', " ").split_whitespace().map(str::to_owned).collect();
        let key = (tok[0].clone(), tok[2].trim_matches('\'').to_owned());
        if !records.contains_key(&key) {
            order.push(key.clone());
        }
        records.entry(key).or_default().push(rec.to_owned());
    }
    let model = |rec: &str| -> String {
        rec.replace(',', " ").split_whitespace().nth(1).unwrap().trim_matches('\'').to_owned()
    };
    let mut seen: Vec<Vec<String>> = Vec::new();
    let mut cases = Vec::new();
    for unit in order {
        let Some(g) = gens.get(&unit) else { continue };
        if g[14] != "1" {
            continue;
        }
        let mut key: Vec<String> = records[&unit].iter().map(|r| model(r)).collect();
        key.sort();
        if seen.contains(&key) {
            continue;
        }
        seen.push(key.clone());
        let num = |i: usize| -> f64 { g[i].parse().unwrap() };
        let (pg, mbase, zr, zx, vs) = (num(2), num(8), num(9), num(10), num(6));
        let mut named = key.clone();
        named.sort_by_key(|m| (m != "GENROU" && m != "REGCA1", m.clone()));
        let id = format!(
            "smib_{}",
            named.iter().map(|m| m.to_ascii_lowercase()).collect::<Vec<_>>().join("_")
        );
        let x_line = 0.1 * 100.0 / mbase;
        let mut r = String::new();
        r.push_str("0,   100.00,  32, 0, 1, 60.00     / SMIB of a WECC 240 unit\n\n\n");
        r.push_str("   101,'INF BUS', 230.0000,3,   1,   1,   1,1.00000,   0.0000\n");
        r.push_str(&format!("   102,'UNIT',    230.0000,2,   1,   1,   1,{vs:.5},   0.0000\n"));
        r.push_str(" 0 /End of Bus data, Begin Load data\n 0 /End of Load data, Begin Fixed shunt data\n");
        r.push_str(" 0 /End of Fixed shunt data, Begin Generator data\n");
        r.push_str(&format!(
            "   101,'1 ', {:10.3},     0.000, 99999.000, -99999.000,1.00000,     0,    100.000, \
             0.00000E+0, 1.00000E-5, 0.00000E+0, 0.00000E+0,1.00000,1,  100.0, 99999.000, -99999.000,   1,1.0000\n",
            -pg
        ));
        r.push_str(&format!(
            "   102,'1 ', {pg:10.3},     0.000, {:10.3}, {:10.3},{vs:.5},     0, {mbase:10.3}, {}, {}, \
             0.00000E+0, 0.00000E+0,1.00000,1,  100.0, {:10.3}, {:10.3},   1,1.0000\n",
            num(4),
            num(5),
            sci5(zr),
            sci5(zx),
            num(16),
            num(17)
        ));
        r.push_str(" 0 /End of Generator data, Begin Branch data\n");
        for ckt in ["1", "2"] {
            r.push_str(&format!(
                "   101,    102,'{ckt} ', 0.00000E+0, {},   0.00000,  9999.00,  9999.00,  9999.00,  \
                 0.00000,  0.00000,  0.00000,  0.00000,1,1,   0.00,   1,1.0000\n",
                sci5(x_line)
            ));
        }
        r.push_str(SMIB_TAIL);
        let mut d = String::from("    101 'GENCLS' 1     0.0000       0.0000      /\n");
        for rec in &records[&unit] {
            let name = rec.split_whitespace().nth(1).unwrap();
            let rest = rec[rec.find(name).unwrap() + name.len()..].trim_start();
            let params = rest.split_once(char::is_whitespace).map_or("", |(_, p)| p);
            d.push_str(&format!("    102 {name} 1 {params} /\n"));
        }
        cases.push(SmibCase {
            id,
            models: key,
            unit: (unit.0.parse().unwrap(), unit.1),
            raw: r,
            dyr: d,
            x_line,
        });
    }
    cases
}

/// The network and dynamic data of a SMIB case.
#[allow(dead_code)]
pub fn smib_study(case: &SmibCase) -> (powerio::BalancedNetwork, powerio::DynamicCase) {
    let src = powerio::Source::from_memory("case.raw", case.raw.as_bytes().to_vec()).unwrap();
    let module = powerio::parse(src).unwrap_or_else(|e| panic!("{}: {e}", case.id));
    let powerio::PioValue::BalancedNetwork(net) = module.value() else {
        panic!("{}: not a balanced network", case.id)
    };
    let dyr = powerio::parse_dyr(&case.dyr).unwrap_or_else(|e| panic!("{}: {e}", case.id));
    (net.clone(), dyr)
}

/// `raw` (a SMIB case) with `line` as its load record.
#[allow(dead_code)]
pub fn with_load(raw: &str, line: &str) -> String {
    raw.replacen(
        " 0 /End of Bus data, Begin Load data\n",
        &format!(" 0 /End of Bus data, Begin Load data\n{line}\n"),
        1,
    )
}
