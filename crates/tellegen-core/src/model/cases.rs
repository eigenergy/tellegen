//! Embedded MATPOWER test fixtures shared across the crate's unit tests, so the
//! tests carry their own network data and run without the `data/` directory.

#[cfg(feature = "sensitivity")]
use super::AcNetwork;
use super::DcNetwork;

/// Shared 3-bus test fixture: bus 1 slack with a generator, bus 3 PV with a
/// generator, bus 2 a pure 90 MW load. Three identical lines (r = 0.01,
/// x = 0.1). Standard MATPOWER column widths.
pub(crate) const CASE3: &str = "\
function mpc = case3test
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0  0  0 0 1 1 0 230 1 1.1 0.9;
 2 1 90 30 0 0 1 1 0 230 1 1.1 0.9;
 3 2 0  0  0 0 1 1 0 230 1 1.1 0.9;
];
mpc.gen = [
 1 0  0 300 -300 1 100 1 250 10 0 0 0 0 0 0 0 0 0 0 0;
 3 60 0 300 -300 1 100 1 270 10 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 2 0.01 0.1 0 250 250 250 0 0 1 -360 360;
 1 3 0.01 0.1 0 250 250 250 0 0 1 -360 360;
 2 3 0.01 0.1 0 250 250 250 0 0 1 -360 360;
];
mpc.gencost = [
 2 0 0 3 0.11  5   0;
 2 0 0 3 0.085 1.2 0;
];
";

/// Parse and build the shared 3-bus fixture.
pub(crate) fn parse_case3() -> DcNetwork {
    let net = powerio::parse_str(CASE3, "matpower")
        .expect("parse case3")
        .network;
    DcNetwork::from_network(&net).expect("build DcNetwork")
}

/// The standard MATPOWER 9-bus, 3-generator case (Chow / WSCC), embedded so the
/// AC tests carry their own data. Bus 1 is the slack; branches 1-4, 3-6, 8-2 are
/// pure reactances (transformers with no off-nominal tap), the rest carry line
/// charging — a small case that still exercises the full pi-model assembly.
#[cfg(feature = "sensitivity")]
const CASE9: &str = "\
function mpc = case9
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [
 1 3 0   0  0 0 1 1 0 345 1 1.1 0.9;
 2 2 0   0  0 0 1 1 0 345 1 1.1 0.9;
 3 2 0   0  0 0 1 1 0 345 1 1.1 0.9;
 4 1 0   0  0 0 1 1 0 345 1 1.1 0.9;
 5 1 90  30 0 0 1 1 0 345 1 1.1 0.9;
 6 1 0   0  0 0 1 1 0 345 1 1.1 0.9;
 7 1 100 35 0 0 1 1 0 345 1 1.1 0.9;
 8 1 0   0  0 0 1 1 0 345 1 1.1 0.9;
 9 1 125 50 0 0 1 1 0 345 1 1.1 0.9;
];
mpc.gen = [
 1 72.3 27.03  300 -300 1.04  100 1 250 10 0 0 0 0 0 0 0 0 0 0 0;
 2 163  6.54   300 -300 1.025 100 1 300 10 0 0 0 0 0 0 0 0 0 0 0;
 3 85   -10.95 300 -300 1.025 100 1 270 10 0 0 0 0 0 0 0 0 0 0 0;
];
mpc.branch = [
 1 4 0      0.0576 0     250 250 250 0 0 1 -360 360;
 4 5 0.017  0.092  0.158 250 250 250 0 0 1 -360 360;
 5 6 0.039  0.17   0.358 150 150 150 0 0 1 -360 360;
 3 6 0      0.0586 0     300 300 300 0 0 1 -360 360;
 6 7 0.0119 0.1008 0.209 150 150 150 0 0 1 -360 360;
 7 8 0.0085 0.072  0.149 250 250 250 0 0 1 -360 360;
 8 2 0      0.0625 0     250 250 250 0 0 1 -360 360;
 8 9 0.032  0.161  0.306 250 250 250 0 0 1 -360 360;
 9 4 0.01   0.085  0.176 250 250 250 0 0 1 -360 360;
];
mpc.gencost = [
 2 1500 0 3 0.11   5   150;
 2 2000 0 3 0.085  1.2 600;
 2 3000 0 3 0.1225 1   335;
];
";

/// Parse and build the 9-bus fixture as an [`AcNetwork`].
#[cfg(feature = "sensitivity")]
pub(crate) fn parse_case9_ac() -> AcNetwork {
    let net = powerio::parse_str(CASE9, "matpower")
        .expect("parse case9")
        .network;
    AcNetwork::from_network(&net).expect("build AcNetwork")
}

/// Parse and build the shared 3-bus fixture as an [`AcNetwork`].
#[cfg(feature = "conic")]
pub(crate) fn parse_case3_ac() -> AcNetwork {
    let net = powerio::parse_str(CASE3, "matpower")
        .expect("parse case3")
        .network;
    AcNetwork::from_network(&net).expect("build AcNetwork")
}
