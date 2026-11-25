// The options under which tellegen simulates what PowerSimulationsDynamics.jl
// 0.16.2 simulates. Included by the tests that compare with it.

/// What PowerSimulationsDynamics.jl 0.16.2 simulates, so the comparison
/// is like with like where tellegen's defaults differ: the units start at
/// their voltage setpoints whatever reactive power that takes (its power
/// flow does not enforce reactive limits; tellegen's default does, as its
/// static study), the loads keep their law at any voltage (no PSS/E
/// `PQBRAK` conversion), the quadratic saturation keeps its formula below
/// the knee `A` (PSS/E: 0), and a renewable plant's converter ramp limits
/// act on its lag's error.
#[allow(dead_code)]
fn psid_options() -> RmsOptions {
    RmsOptions {
        reactive_limits: false,
        psid_saturation: true,
        psid_converter_ramp: true,
        low_voltage_load: LowVoltageLoad::Exact,
        // PowerSimulationsDynamics.jl switches a non-windup limit within the
        // step; held limits switch one step later.
        freeze_limits: false,
        ..RmsOptions::default()
    }
}
