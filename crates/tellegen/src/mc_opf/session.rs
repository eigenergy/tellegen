//! Revisioned native sessions. Numerical edits recompile embedded constants;
//! the compiled model is retained between solves of unchanged physical inputs.
use super::*;
use num_complex::Complex64;
use powerio_prob::{MulticonductorActiveConstraints, MulticonductorOperatingPointBuilder};

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

/// Physical edits, addressed by canonical identity. Vectors follow the named
/// equipment's canonical order. A batch is validated and committed atomically.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum McOpfEdit {
    /// Nominal powers per load coil (W/var), preserving its voltage law.
    /// These are not the terminal powers used in an operating-point seed.
    LoadPower {
        load: String,
        p_w: Vec<f64>,
        q_var: Vec<f64>,
    },
    /// Replace per-coil available active-power caps (W), and set p_avail to
    /// their sum for controls. None clears both p_max and p_avail. Other
    /// capability fields stay unchanged; no allocation across coils is inferred.
    InverterAvailability {
        inverter: String,
        p_max_w: Option<Vec<f64>>,
    },
    /// Canonical $/kWh cost: scalar or one entry per coil; None removes it.
    GeneratorCost {
        generator: String,
        cost_per_kwh: Option<Vec<f64>>,
    },
    SourceCost {
        source: String,
        cost_per_kwh: Option<Vec<f64>>,
    },
    /// Replace all four per-coil dispatch bounds. None removes that field.
    GeneratorPowerBounds {
        generator: String,
        p_min_w: Option<Vec<f64>>,
        p_max_w: Option<Vec<f64>>,
        q_min_var: Option<Vec<f64>>,
        q_max_var: Option<Vec<f64>>,
    },
    /// Replace per-conductor line overrides. None restores linecode inheritance.
    LineLimits {
        line: String,
        current_a: Option<Vec<f64>>,
        apparent_power_va: Option<Vec<f64>>,
    },
    /// Replace scalar phase-ground bounds. PowerIO's vector fallback and other
    /// voltage families remain unchanged when a scalar is removed.
    BusVoltageBounds {
        bus: String,
        minimum_v: Option<f64>,
        maximum_v: Option<f64>,
    },
    ConstraintSelection {
        constraints: MulticonductorActiveConstraints,
    },
}

/// Select a numerical start; neither choice changes the declared instance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfStart {
    /// Use the instance's explicit initial point, or the ordinary cold start.
    Declared,
    /// Carry the last accepted primal, falling back to Declared if unavailable.
    #[default]
    LastAccepted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McOpfStartUsed {
    Declared,
    /// Independent columns carried by identity, then dependent columns rebuilt.
    PreviousPrimal,
    /// SI voltages/taps projected after a numerical base change.
    PreviousOperatingPoint,
}

/// An accepted result labelled with the input revision and solve attempt.
/// Historical results remain available explicitly, never as current detail.
#[derive(Clone, Debug)]
pub struct McOpfSessionResult {
    pub revision: u64,
    pub attempt: u64,
    pub compilation: u64,
    pub start: McOpfStartUsed,
    pub result: McOpfResult,
}

/// Retains the current instance, compiled IVR model and last accepted primal.
/// Each optimization uses a fresh POUNCE application; solver multipliers and
/// factorizations are not retained. Topology/connection edits require a new
/// session. Physical edits always reprepare/recompile, including zero ratings.
#[derive(Debug)]
pub struct McOpfSession {
    instance: Arc<McAcOpfInstance>,
    options: McOpfOptions,
    model: model::Model,
    fingerprint: String,
    revision: u64,
    attempts: u64,
    compilations: u64,
    accepted: Option<McOpfSessionResult>,
    primal: BTreeMap<String, f64>,
    primal_names_unique: bool,
}

impl McOpfSession {
    /// Prepare and compile without solving. Use `solve` to obtain the first
    /// accepted result; an infeasible instance can still be edited beforehand.
    pub fn new(instance: McAcOpfInstance, options: McOpfOptions) -> Result<Self, McOpfError> {
        options.validate()?;
        let (model, fingerprint) = compile(&instance, &options)?;
        let instance = initial::canonical_instance(Arc::new(instance))?;
        Ok(Self {
            instance,
            options,
            model,
            fingerprint,
            revision: 0,
            attempts: 0,
            compilations: 1,
            accepted: None,
            primal: BTreeMap::new(),
            primal_names_unique: true,
        })
    }

    pub fn instance(&self) -> &McAcOpfInstance {
        &self.instance
    }
    pub fn options(&self) -> &McOpfOptions {
        &self.options
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn solve_count(&self) -> u64 {
        self.attempts
    }
    /// Successful model compilations, including construction and committed edits.
    pub fn compilation_count(&self) -> u64 {
        self.compilations
    }
    pub fn last_accepted(&self) -> Option<&McOpfSessionResult> {
        self.accepted.as_ref()
    }

    /// None after a committed edit or any unsuccessful solve attempt, even if
    /// an older accepted result has the same physical input revision.
    pub fn current_result(&self) -> Option<&McOpfSessionResult> {
        self.accepted
            .as_ref()
            .filter(|r| r.revision == self.revision && r.attempt == self.attempts)
    }

    /// Apply a batch to a candidate instance and compile before committing it.
    /// Failure leaves input, revision, compiled cache and accepted state intact.
    pub fn apply_edits(&mut self, edits: &[McOpfEdit]) -> Result<u64, McOpfError> {
        if edits.is_empty() {
            return Ok(self.revision);
        }
        let mut net = self.instance.network().clone();
        let mut constraints = self.instance.constraints().clone();
        for edit in edits {
            apply_edit(&mut net, &mut constraints, edit)?;
        }
        let candidate = (*self.instance)
            .clone()
            .with_network(net)
            .map_err(|e| McOpfError::from_powerio(McOpfStage::Input, e))?
            .with_constraints(constraints);
        let (model, fingerprint) = compile(&candidate, &self.options)?;
        let revision = increment(self.revision)?;
        let compilations = increment(self.compilations)?;
        self.instance = Arc::new(candidate);
        self.model = model;
        self.fingerprint = fingerprint;
        self.revision = revision;
        self.compilations = compilations;
        Ok(revision)
    }

    /// Solver/acceptance option changes invalidate current detail. Only working
    /// base changes require recompilation; they use SI warm-start projection.
    pub fn set_options(&mut self, options: McOpfOptions) -> Result<u64, McOpfError> {
        options.validate()?;
        if options == self.options {
            return Ok(self.revision);
        }
        let revision = increment(self.revision)?;
        if options.voltage_base_v != self.options.voltage_base_v
            || options.power_base_va != self.options.power_base_va
        {
            let (model, fingerprint) = compile(&self.instance, &options)?;
            let compilations = increment(self.compilations)?;
            self.model = model;
            self.fingerprint = fingerprint;
            self.compilations = compilations;
        }
        self.options = options;
        self.revision = revision;
        Ok(revision)
    }

    /// Solve the current revision, warm-starting from the last accepted result
    /// when available. Failed attempts never overwrite that accepted result.
    pub fn solve(&mut self) -> Result<&McOpfSessionResult, McOpfError> {
        self.solve_cancellable(McOpfStart::LastAccepted, None)
    }

    pub fn solve_cancellable(
        &mut self,
        start: McOpfStart,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<&McOpfSessionResult, McOpfError> {
        self.attempts = increment(self.attempts)?;
        cancelled(cancel.as_deref(), McOpfStage::Input)?;
        let mut profile = self.options.collect_profile.then(McOpfProfile::default);
        let mut stage = Instant::now();
        let mut model = self.model.clone();
        record_stage(&mut profile, &mut stage, "session_model_clone");
        let (used, diagnostics) = self.initialize(&mut model, start)?;
        record_stage(&mut profile, &mut stage, "initial_point");
        let (result, x) = solve_prepared(
            self.instance.clone(),
            &self.options,
            cancel.clone(),
            PreparedSolve {
                model,
                fingerprint: self.fingerprint.clone(),
                initial_diagnostics: diagnostics,
                profile,
                stage,
            },
        )?;
        cancelled(cancel.as_deref(), McOpfStage::Validation)?;
        self.primal = self
            .model
            .problem
            .var_names
            .iter()
            .cloned()
            .zip(x)
            .collect();
        self.primal_names_unique = self.primal.len() == self.model.problem.var_names.len();
        self.accepted = Some(McOpfSessionResult {
            revision: self.revision,
            attempt: self.attempts,
            compilation: self.compilations,
            start: used,
            result,
        });
        Ok(self.accepted.as_ref().expect("just stored accepted result"))
    }

    fn initialize(
        &self,
        model: &mut model::Model,
        start: McOpfStart,
    ) -> Result<(McOpfStartUsed, Vec<McOpfDiagnostic>), McOpfError> {
        let Some(accepted) = self
            .accepted
            .as_ref()
            .filter(|_| start == McOpfStart::LastAccepted)
        else {
            return initial::apply(model, &self.instance).map(|d| (McOpfStartUsed::Declared, d));
        };
        let options = &accepted.result.options;
        let same_bases = options.voltage_base_v == self.options.voltage_base_v
            && options.power_base_va == self.options.power_base_va;
        // Display names can contain separators supplied by the input. If they
        // collide, use the explicit terminal identities in the physical report.
        let unique_names = model
            .problem
            .var_names
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == model.problem.var_names.len();
        let projected = if same_bases && self.primal_names_unique && unique_names {
            let mut x = model.problem.x0.clone();
            for (i, name) in model.problem.var_names.iter().enumerate() {
                if let Some(value) = self.primal.get(name) {
                    x[i] = *value;
                }
            }
            initial::refresh(model, &mut x).map(|()| {
                model.problem.x0 = x;
                (McOpfStartUsed::PreviousPrimal, Vec::new())
            })
        } else {
            voltage_tap_point(&self.instance, &accepted.result, model).and_then(|instance| {
                initial::apply(model, &instance)
                    .map(|d| (McOpfStartUsed::PreviousOperatingPoint, d))
            })
        };
        match projected {
            Ok(result) => Ok(result),
            Err(error) => {
                // A new positive log-voltage domain, for example, can make a
                // previously accepted zero-voltage point unusable as a seed.
                *model = self.model.clone();
                let mut diagnostics = initial::apply(model, &self.instance)?;
                diagnostics.push(McOpfDiagnostic {
                    code: "MC_OPF.SESSION_WARM_START_DISCARDED".into(),
                    severity: powerio::DiagnosticSeverity::Remark,
                    message: error.to_string(),
                    target: None,
                });
                Ok((McOpfStartUsed::Declared, diagnostics))
            }
        }
    }
}

fn compile(
    instance: &McAcOpfInstance,
    options: &McOpfOptions,
) -> Result<(model::Model, String), McOpfError> {
    let prep = initial::prepare(
        instance,
        &McAcOpfAssemblyOptions::new(options.voltage_base_v, options.power_base_va),
    )?;
    let fingerprint = result::fingerprint(&prep)?;
    let model = model::compile_with_parameters(prep, instance.network())
        .map_err(|e| McOpfError::new(McOpfErrorKind::InvalidInput, McOpfStage::Compilation, e))?;
    Ok((model, fingerprint))
}

fn increment(value: u64) -> Result<u64, McOpfError> {
    value
        .checked_add(1)
        .ok_or_else(|| invalid("session", "revision/counter exhausted"))
}
fn invalid(target: &str, message: &str) -> McOpfError {
    let mut error = McOpfError::new(
        McOpfErrorKind::InvalidInput,
        McOpfStage::Input,
        format!("session {target}: {message}"),
    );
    error.diagnostics.push(McOpfDiagnostic {
        code: "MC_OPF.SESSION_EDIT_INVALID".into(),
        severity: powerio::DiagnosticSeverity::Error,
        message: error.message.clone(),
        target: Some(target.into()),
    });
    error
}

fn voltage_tap_point(
    instance: &McAcOpfInstance,
    previous: &McOpfResult,
    model: &model::Model,
) -> Result<McAcOpfInstance, McOpfError> {
    let net = instance.network();
    let voltages: BTreeMap<_, _> = previous
        .terminals
        .iter()
        .map(|v| {
            (
                (v.terminal.bus.as_str(), v.terminal.terminal.as_str()),
                Complex64::new(v.voltage_v[0], v.voltage_v[1]),
            )
        })
        .collect();
    let mut vm = Vec::new();
    let mut va = Vec::new();
    for bus in net.buses() {
        for terminal in &bus.terminals {
            let v = voltages
                .get(&(bus.id.as_str(), terminal.as_str()))
                .ok_or_else(|| invalid(&bus.id, "warm-start terminal is missing"))?;
            vm.push(v.norm());
            va.push(v.arg());
        }
    }
    let mut builder = MulticonductorOperatingPointBuilder::for_point(net.clone())
        .terminal_voltage_magnitudes(vm)
        .terminal_voltage_angles(va);
    if model.transformer_tap.iter().any(Option::is_some) {
        let taps = model
            .prep
            .transformers
            .iter()
            .map(|t| {
                previous
                    .transformers
                    .iter()
                    .find(|r| r.identity == t.identity)
                    .and_then(|r| r.tap)
                    .unwrap_or(t.tap_nominal)
            })
            .collect();
        builder = builder.transformer_taps(taps);
    }
    let point = builder
        .build_point()
        .map_err(|e| McOpfError::from_powerio(McOpfStage::Input, e))?;
    Ok(instance.clone().with_initial_point(point))
}

fn named<'a, T>(
    items: &'a mut [T],
    name: &str,
    identity: impl Fn(&T) -> &str,
) -> Result<&'a mut T, McOpfError> {
    let mut matches = items.iter_mut().filter(|item| identity(item) == name);
    let item = matches
        .next()
        .ok_or_else(|| invalid(name, "unknown component identity"))?;
    if matches.next().is_some() {
        return Err(invalid(name, "ambiguous component identity"));
    }
    Ok(item)
}
fn numbers(target: &str, values: &[f64], nonnegative: bool) -> Result<(), McOpfError> {
    if values
        .iter()
        .any(|v| !v.is_finite() || (nonnegative && *v < 0.0))
    {
        return Err(invalid(
            target,
            "values must be finite and in the field's physical domain",
        ));
    }
    Ok(())
}
fn vector(target: &str, values: &[f64], n: usize, nonnegative: bool) -> Result<(), McOpfError> {
    if values.len() != n {
        return Err(invalid(
            target,
            "vector length differs from the canonical equipment axis",
        ));
    }
    numbers(target, values, nonnegative)
}
fn optional_vector(
    target: &str,
    values: &Option<Vec<f64>>,
    n: usize,
    nonnegative: bool,
) -> Result<(), McOpfError> {
    if let Some(values) = values {
        vector(target, values, n, nonnegative)?;
    }
    Ok(())
}
fn cost(target: &str, values: &Option<Vec<f64>>, n: usize) -> Result<(), McOpfError> {
    if let Some(values) = values {
        if values.len() != 1 && values.len() != n {
            return Err(invalid(target, "cost must be scalar or per coil"));
        }
        numbers(target, values, false)?;
    }
    Ok(())
}
fn ordered_bounds(
    target: &str,
    lo: &Option<Vec<f64>>,
    hi: &Option<Vec<f64>>,
) -> Result<(), McOpfError> {
    if let (Some(lo), Some(hi)) = (lo, hi) {
        if lo.iter().zip(hi).any(|(lo, hi)| lo > hi) {
            return Err(invalid(target, "minimum exceeds maximum"));
        }
    }
    Ok(())
}

fn apply_edit(
    net: &mut powerio::MulticonductorNetwork,
    constraints: &mut MulticonductorActiveConstraints,
    edit: &McOpfEdit,
) -> Result<(), McOpfError> {
    match edit {
        McOpfEdit::LoadPower { load, p_w, q_var } => {
            let d = named(net.loads_mut(), load, |d| &d.name)?;
            vector(load, p_w, d.p_nom.len(), false)?;
            vector(load, q_var, d.q_nom.len(), false)?;
            d.p_nom.clone_from(p_w);
            d.q_nom.clone_from(q_var);
        }
        McOpfEdit::InverterAvailability { inverter, p_max_w } => {
            let d = named(net.ibrs_mut(), inverter, |d| &d.name)?;
            optional_vector(inverter, p_max_w, d.s_max.len(), true)?;
            let total = p_max_w.as_ref().map(|v| v.iter().sum::<f64>());
            if total.is_some_and(|v| !v.is_finite()) {
                return Err(invalid(inverter, "total availability is not finite"));
            }
            d.p_max.clone_from(p_max_w);
            d.p_avail = total;
        }
        McOpfEdit::GeneratorCost {
            generator,
            cost_per_kwh,
        } => {
            let d = named(net.generators_mut(), generator, |d| &d.name)?;
            cost(generator, cost_per_kwh, d.p_nom.len())?;
            d.cost.clone_from(cost_per_kwh);
        }
        McOpfEdit::SourceCost {
            source,
            cost_per_kwh,
        } => {
            let d = named(net.sources_mut(), source, |d| &d.name)?;
            // Source voltage columns include the neutral; preparation checks
            // the exact active-phase count for non-scalar costs.
            if let Some(values) = cost_per_kwh {
                if values.is_empty() {
                    return Err(invalid(source, "cost cannot be empty"));
                }
                numbers(source, values, false)?;
            }
            d.energy_cost_rate.clone_from(cost_per_kwh);
        }
        McOpfEdit::GeneratorPowerBounds {
            generator,
            p_min_w,
            p_max_w,
            q_min_var,
            q_max_var,
        } => {
            let d = named(net.generators_mut(), generator, |d| &d.name)?;
            for values in [p_min_w, p_max_w, q_min_var, q_max_var] {
                optional_vector(generator, values, d.p_nom.len(), false)?;
            }
            ordered_bounds(generator, p_min_w, p_max_w)?;
            ordered_bounds(generator, q_min_var, q_max_var)?;
            d.p_min.clone_from(p_min_w);
            d.p_max.clone_from(p_max_w);
            d.q_min.clone_from(q_min_var);
            d.q_max.clone_from(q_max_var);
        }
        McOpfEdit::LineLimits {
            line,
            current_a,
            apparent_power_va,
        } => {
            let d = named(net.lines_mut(), line, |d| &d.name)?;
            for values in [current_a, apparent_power_va] {
                optional_vector(line, values, d.terminal_map_from.len(), true)?;
            }
            d.i_max.clone_from(current_a);
            d.s_max.clone_from(apparent_power_va);
        }
        McOpfEdit::BusVoltageBounds {
            bus,
            minimum_v,
            maximum_v,
        } => {
            numbers(
                bus,
                &minimum_v
                    .iter()
                    .chain(maximum_v.iter())
                    .copied()
                    .collect::<Vec<_>>(),
                true,
            )?;
            if let (Some(lo), Some(hi)) = (minimum_v, maximum_v) {
                if lo > hi {
                    return Err(invalid(bus, "minimum exceeds maximum"));
                }
            }
            let d = named(net.buses_mut(), bus, |d| &d.id)?;
            d.v_min = *minimum_v;
            d.v_max = *maximum_v;
        }
        McOpfEdit::ConstraintSelection { constraints: value } => constraints.clone_from(value),
    }
    Ok(())
}
