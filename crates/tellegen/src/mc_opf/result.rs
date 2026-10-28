//! Complete result transport through PowerIO's public extension mechanism.
use super::*;
use powerio_matrix::McAcOpfPreparation;
use sha2::{Digest, Sha256};

/// Versioned Tellegen result data accompanying a canonical McAcOpfSolution.
pub const MC_OPF_RESULT_EXTENSION: &str = "org.eigenergy.tellegen.mc_opf";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McOpfTerminalId {
    pub bus: String,
    pub terminal: String,
}
impl McOpfTerminalId {
    pub(super) fn at(prep: &McAcOpfPreparation, index: usize) -> Self {
        let t = &prep.terminals[index];
        Self {
            bus: t.bus.clone(),
            terminal: t.terminal.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McOpfTerminalVoltage {
    pub terminal: McOpfTerminalId,
    /// Rectangular [real, imaginary] earth-referenced voltage, volts.
    pub voltage_v: [f64; 2],
}

/// Zero-based local coil index and its electrical incidence. An absent negative
/// terminal denotes physical ground, not an omitted neutral label.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McOpfCoilAxis {
    pub index: usize,
    pub positive: McOpfTerminalId,
    pub negative: Option<McOpfTerminalId>,
}
impl McOpfCoilAxis {
    pub(super) fn new(prep: &McAcOpfPreparation, index: usize, p: usize, n: Option<usize>) -> Self {
        Self {
            index,
            positive: McOpfTerminalId::at(prep, p),
            negative: n.map(|n| McOpfTerminalId::at(prep, n)),
        }
    }
}

/// One prepared transformer rating port, oriented into the transformer.
/// `name` retains PowerIO's winding/port identity (for example `w0:0`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McOpfPortResult {
    pub name: String,
    pub positive: McOpfTerminalId,
    pub negative: Option<McOpfTerminalId>,
    pub current_a: [f64; 2],
    pub power_va: [f64; 2],
    pub current_limit_a: Option<f64>,
    pub apparent_limit_va: Option<f64>,
}

/// Separate admittance stamps, including capacitor, core and grounding stamps.
/// Currents enter the shunt. Do not add rating ports to these physical ledgers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McOpfShuntResult {
    pub identity: String,
    pub terminals: Vec<McOpfTerminalId>,
    pub current_a: Vec<[f64; 2]>,
    pub power_va: Vec<[f64; 2]>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    version: u32,
    #[serde(default)]
    duals: Option<McOpfDualReport>,
    options: McOpfOptions,
    terminals: Vec<McOpfTerminalVoltage>,
    branches: Vec<McOpfBranchResult>,
    devices: Vec<McOpfDeviceResult>,
    transformers: Vec<McOpfTransformerResult>,
    shunts: Vec<McOpfShuntResult>,
    residuals: McOpfResiduals,
    diagnostics: Vec<McOpfDiagnostic>,
    iterations: i32,
    fingerprint: String,
    profile: Option<McOpfProfile>,
}

pub(super) fn fingerprint(prep: &McAcOpfPreparation) -> Result<String, McOpfError> {
    let bytes = serde_json::to_vec(prep).map_err(|e| invalid(e.to_string()))?;
    Ok(
        Sha256::digest([b"tellegen/mc-ivr-v2\0".as_slice(), &bytes].concat())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}
fn invalid(message: impl Into<String>) -> McOpfError {
    McOpfError::new(McOpfErrorKind::InvalidInput, McOpfStage::Import, message)
}

impl McOpfResult {
    /// Export the canonical solution and complete, versioned physical report.
    pub fn to_module_json(&self) -> Result<String, McOpfError> {
        let value = PioValue::McAcOpfSolution(self.solution.clone());
        let mut module = match &self.module {
            Some(module) => module.clone().map_value(|_| value).sever_source(),
            None => PioModule::new(value),
        };
        self.insert_report(&mut module)?;
        crate::ir::serialize_module(&module).map_err(|e| {
            McOpfError::new(McOpfErrorKind::SerializationFailure, McOpfStage::Export, e)
        })
    }

    fn report(&self) -> Report {
        Report {
            version: 5,
            duals: self.duals.clone(),
            options: self.options.clone(),
            terminals: self.terminals.clone(),
            branches: self.branches.clone(),
            devices: self.devices.clone(),
            transformers: self.transformers.clone(),
            shunts: self.shunts.clone(),
            residuals: self.residuals.clone(),
            diagnostics: self.diagnostics.clone(),
            iterations: self.iterations,
            fingerprint: self.fingerprint.clone(),
            profile: self.profile.clone(),
        }
    }

    pub(super) fn verified_dual_model(&self) -> Result<Option<model::Model>, McOpfError> {
        self.options.validate()?;
        self.report().verify(&self.solution)
    }

    pub(super) fn insert_report(&self, module: &mut PioModule<PioValue>) -> Result<(), McOpfError> {
        let report = self.report();
        // JSON turns nonfinite floats into null. Round-tripping the typed report
        // refuses those nulls in required quantities; check optional floats too.
        let encode = || -> Result<serde_json::Value, McOpfError> {
            let value = serde_json::to_value(&report).map_err(|e| invalid(e.to_string()))?;
            serde_json::from_value::<Report>(value.clone()).map_err(|e| invalid(e.to_string()))?;
            report.verify(&self.solution)?;
            Ok(value)
        };
        let value = encode().map_err(|mut e| {
            e.kind = McOpfErrorKind::SerializationFailure;
            e.stage = McOpfStage::Export;
            e
        })?;
        module
            .insert_extension(MC_OPF_RESULT_EXTENSION, value)
            .map_err(|e| McOpfError::from_powerio(McOpfStage::Export, e))?;
        Ok(())
    }

    /// Read a complete saved report. Refuses missing/unknown versions and
    /// inconsistent identities, shapes, bases or canonical voltage columns.
    /// Reading does not rerun the optimizer or certify an external solution.
    pub fn from_module_json(input: &str) -> Result<Self, McOpfError> {
        let module = crate::ir::deserialize_module_with_diagnostics(input)
            .map_err(|e| McOpfError::from_powerio(McOpfStage::Import, e))?;
        validate_mc_opf_input_diagnostics(&module)?;
        let PioValue::McAcOpfSolution(solution) = module.value() else {
            return Err(invalid("expected a PowerIO McAcOpfSolution module"));
        };
        let value = module
            .extensions()
            .get(MC_OPF_RESULT_EXTENSION)
            .ok_or_else(|| invalid("module has no complete Tellegen MC OPF report"))?;
        let mut report: Report =
            serde_json::from_value(value.clone()).map_err(|e| invalid(e.to_string()))?;
        report.verify(solution)?;
        if matches!(report.version, 2..=4) {
            if let Some(old) = &report.duals {
                // Verify legacy claims with their original parameter map first;
                // then recover newly supported derivatives from canonical input.
                let prep = initial::prepare(
                    solution.instance(),
                    &McAcOpfAssemblyOptions::new(
                        report.options.voltage_base_v,
                        report.options.power_base_va,
                    ),
                )?;
                let model = model::compile_with_parameters(prep, solution.instance().network())
                    .map_err(invalid)?;
                let lambda: Vec<_> = old.constraints.iter().map(|r| r.multiplier).collect();
                report.duals = Some(
                    duals::analyze(
                        &model,
                        &old.primal,
                        &lambda,
                        &old.lower_bound_multipliers,
                        &old.upper_bound_multipliers,
                    )
                    .map_err(invalid)?,
                );
            }
        }
        Ok(Self {
            duals: report.duals,
            module: Some(module.clone()),
            solution: solution.clone(),
            options: report.options,
            terminals: report.terminals,
            branches: report.branches,
            devices: report.devices,
            transformers: report.transformers,
            shunts: report.shunts,
            residuals: report.residuals,
            diagnostics: report.diagnostics,
            iterations: report.iterations,
            fingerprint: report.fingerprint,
            profile: report.profile,
        })
    }
}

impl Report {
    fn verify(&self, solution: &McAcOpfSolution) -> Result<Option<model::Model>, McOpfError> {
        if !matches!(self.version, 1..=5) || (self.version == 1 && self.duals.is_some()) {
            return Err(invalid("unsupported MC OPF report version"));
        }
        let prep = initial::prepare(
            solution.instance(),
            &McAcOpfAssemblyOptions::new(self.options.voltage_base_v, self.options.power_base_va),
        )
        .map_err(|mut e| {
            e.stage = McOpfStage::Import;
            e
        })?;
        if self.fingerprint != fingerprint(&prep)? {
            return Err(invalid(
                "MC OPF report fingerprint does not match its instance and bases",
            ));
        }
        let mut dual_model = None;
        if let Some(duals) = &self.duals {
            if !self.options.collect_duals {
                return Err(invalid("dual report without collection option"));
            }
            let model = if self.version < 3 {
                model::compile(prep.clone())
            } else if self.version == 3 {
                model::compile_with_load_parameters(prep.clone(), solution.instance().network())
            } else if self.version == 4 {
                model::compile_with_inverter_parameters(prep.clone(), solution.instance().network())
            } else {
                model::compile_with_parameters(prep.clone(), solution.instance().network())
            }
            .map_err(invalid)?;
            let checked = duals
                .verify(&model, solution, self.options.acceptance_tolerance)
                .map_err(invalid)?;
            let stored = serde_json::to_value((
                &self.branches,
                &self.devices,
                &self.transformers,
                &self.shunts,
            ))
            .map_err(|e| invalid(e.to_string()))?;
            let recovered = serde_json::to_value((
                &checked.branches,
                &checked.devices,
                &checked.transformers,
                &checked.shunts,
            ))
            .map_err(|e| invalid(e.to_string()))?;
            if !duals::same_json(&stored, &recovered) {
                return Err(invalid("dual primal disagrees with physical ledgers"));
            }
            dual_model = Some(model);
        } else if self.options.collect_duals
            && !self
                .diagnostics
                .iter()
                .any(|d| d.code == "MC_OPF.DUALS_UNAVAILABLE")
        {
            return Err(invalid(
                "requested dual report is missing without a diagnostic",
            ));
        }
        let ids = |indices: &[usize]| -> Vec<_> {
            indices
                .iter()
                .map(|&i| McOpfTerminalId::at(&prep, i))
                .collect()
        };
        let check = |ok: bool, owner: &str| -> Result<(), McOpfError> {
            if ok {
                Ok(())
            } else {
                Err(invalid(format!("inconsistent MC OPF report: {owner}")))
            }
        };
        check(
            self.terminals.len() == prep.terminals.len()
                && self.branches.len() == prep.branches.len()
                && self.devices.len() == prep.devices.len()
                && self.transformers.len() == prep.transformers.len()
                && self.shunts.len() == prep.shunts.len(),
            "table lengths",
        )?;
        check(
            self.iterations >= 0
                && self.options.acceptance_tolerance.is_finite()
                && self.options.acceptance_tolerance > 0.0
                && self.options.objective_scale.is_finite()
                && self.options.objective_scale > 0.0
                && self.options.max_iterations > 0
                && self.options.max_iterations <= i32::MAX as u32
                && *solution.termination() == Termination::Converged
                && solution.objective().is_finite(),
            "solve metadata",
        )?;
        check(
            [
                self.residuals.kcl_pu,
                self.residuals.kvl_pu,
                self.residuals.power_link_pu,
                self.residuals.prescribed_power_pu,
                self.residuals.relative_limit_violation,
                self.residuals.objective_relative_error,
            ]
            .into_iter()
            .all(|r| r.is_finite() && r >= 0.0 && r <= self.options.acceptance_tolerance),
            "accepted residual summary",
        )?;
        for (i, t) in self.terminals.iter().enumerate() {
            check(t.terminal == McOpfTerminalId::at(&prep, i), "terminal axis")?;
            let magnitude = solution
                .terminal_voltage_magnitude(&t.terminal.bus, &t.terminal.terminal)
                .unwrap();
            let angle = solution
                .terminal_voltage_angle(&t.terminal.bus, &t.terminal.terminal)
                .unwrap();
            check(
                t.voltage_v
                    .iter()
                    .zip([magnitude * angle.cos(), magnitude * angle.sin()])
                    .all(|(v, expected)| {
                        v.is_finite()
                            && expected.is_finite()
                            && (v - expected).abs() <= 1e-10 * (1.0 + magnitude)
                    }),
                "canonical voltage columns",
            )?;
        }
        for (r, p) in self.branches.iter().zip(&prep.branches) {
            check(
                r.identity == p.identity
                    && r.open == p.open
                    && r.from_terminals == ids(&p.from)
                    && r.to_terminals == ids(&p.to)
                    && [
                        &r.current_from_a,
                        &r.current_to_a,
                        &r.power_from_va,
                        &r.power_to_va,
                    ]
                    .iter()
                    .all(|a| a.len() == p.from.len()),
                &p.identity,
            )?;
        }
        for (r, p) in self.devices.iter().zip(&prep.devices) {
            let coils: Vec<_> = p
                .coils
                .iter()
                .enumerate()
                .map(|(i, c)| McOpfCoilAxis::new(&prep, i, c.positive, c.negative))
                .collect();
            check(
                r.identity == p.identity
                    && r.kind == p.kind
                    && r.terminals == ids(&p.terminals)
                    && r.coils == coils
                    && r.coil_current_a.len() == coils.len()
                    && r.coil_power_va.len() == coils.len()
                    && r.terminal_current_a.len() == p.terminals.len()
                    && r.terminal_power_va.len() == p.terminals.len(),
                &p.identity,
            )?;
        }
        for (kind, canonical) in [
            (
                powerio_matrix::McOpfDeviceKind::Source,
                solution.source_active_injections(),
            ),
            (
                powerio_matrix::McOpfDeviceKind::Generator,
                solution.generator_active_powers(),
            ),
        ] {
            let reported: Vec<_> = self
                .devices
                .iter()
                .filter(|d| d.kind == kind)
                .flat_map(|d| d.terminal_power_va.iter().map(|p| p[0]))
                .collect();
            check(
                reported.as_slice() == canonical,
                "canonical dispatch columns",
            )?;
        }
        let objective: f64 = self
            .devices
            .iter()
            .zip(&prep.devices)
            .flat_map(|(r, p)| {
                r.coil_power_va
                    .iter()
                    .zip(&p.coils)
                    .map(|(power, coil)| coil.cost * (power[0] / self.options.power_base_va))
            })
            .sum();
        check(
            objective.is_finite()
                && (objective - solution.objective()).abs() <= 1e-10 * (1.0 + objective.abs()),
            "canonical objective",
        )?;
        for (r, p) in self.transformers.iter().zip(&prep.transformers) {
            let coils: Vec<_> = p
                .coils
                .iter()
                .enumerate()
                .map(|(i, c)| McOpfCoilAxis::new(&prep, i, c.positive, c.negative))
                .collect();
            check(
                r.identity == p.identity
                    && r.coils == coils
                    && r.coil_current_a.len() == coils.len()
                    && r.coil_power_va.len() == coils.len()
                    && r.ports.len() == p.ports.len()
                    && r.tap.is_some() == p.tap_bounds.is_some()
                    && r.tap.is_none_or(f64::is_finite),
                &p.identity,
            )?;
            for (port, expected) in r.ports.iter().zip(&p.ports) {
                let base_i = self.options.power_base_va / self.options.voltage_base_v;
                check(
                    port.name == expected.name
                        && port.positive == McOpfTerminalId::at(&prep, expected.positive)
                        && port.negative
                            == expected.negative.map(|n| McOpfTerminalId::at(&prep, n))
                        && port.current_limit_a == expected.current_max.map(|i| i * base_i)
                        && port.apparent_limit_va
                            == expected
                                .apparent_max
                                .map(|s| s * self.options.power_base_va),
                    &p.identity,
                )?;
            }
        }
        for (r, p) in self.shunts.iter().zip(&prep.shunts) {
            check(
                r.identity == p.identity
                    && r.terminals == ids(&p.terminals)
                    && r.current_a.len() == p.terminals.len()
                    && r.power_va.len() == p.terminals.len(),
                &p.identity,
            )?;
        }
        if let Some(linear) = self.profile.as_ref().and_then(|p| p.linear_solver.as_ref()) {
            check(linear.max_fill_ratio.is_none_or(f64::is_finite), "profile")?;
        }
        Ok(dual_model)
    }
}
