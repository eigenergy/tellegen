//! Explicit cold-start DC branch conventions. The legacy default is unchanged.
use powerio::BranchSusceptanceFormula;
use serde::{Deserialize, Serialize};

/// The complete DC convention, including taps and phase shifts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DcFormula {
    /// Legacy Tellegen: series susceptance with shifts, ignoring taps.
    #[default]
    SeriesSusceptance,
    /// MATPOWER / PowerModels DCMP: ignore resistance, include taps and shifts.
    TapAdjustedReactance,
    /// Textbook 1/x, ignoring taps and shifts.
    ReactanceOnly,
    /// Ignore conductance: series susceptance divided by tap, with shifts.
    TapAdjustedSeriesSusceptance,
    /// PowerModels DCP (PGLib DC baseline): ignore taps and shifts.
    SeriesSusceptanceNoPhaseShift,
}

impl From<DcFormula> for BranchSusceptanceFormula {
    fn from(value: DcFormula) -> Self {
        match value {
            DcFormula::SeriesSusceptance => Self::SeriesSusceptance,
            DcFormula::TapAdjustedReactance => Self::TapAdjustedReactance,
            DcFormula::ReactanceOnly => Self::ReactanceOnly,
            DcFormula::TapAdjustedSeriesSusceptance => Self::TapAdjustedSeriesSusceptance,
            DcFormula::SeriesSusceptanceNoPhaseShift => Self::SeriesSusceptanceNoPhaseShift,
        }
    }
}

impl TryFrom<BranchSusceptanceFormula> for DcFormula {
    type Error = String;
    fn try_from(value: BranchSusceptanceFormula) -> Result<Self, Self::Error> {
        match value {
            BranchSusceptanceFormula::SeriesSusceptance => Ok(Self::SeriesSusceptance),
            BranchSusceptanceFormula::TapAdjustedReactance => Ok(Self::TapAdjustedReactance),
            BranchSusceptanceFormula::ReactanceOnly => Ok(Self::ReactanceOnly),
            BranchSusceptanceFormula::TapAdjustedSeriesSusceptance => {
                Ok(Self::TapAdjustedSeriesSusceptance)
            }
            BranchSusceptanceFormula::SeriesSusceptanceNoPhaseShift => {
                Ok(Self::SeriesSusceptanceNoPhaseShift)
            }
            _ => Err("unsupported DC branch susceptance formula".into()),
        }
    }
}

/// Options for a retained Study. An omitted convention inherits a typed instance.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StudyOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_susceptance_formula: Option<DcFormula>,
}

pub(crate) fn validate(
    formulation: crate::Problem,
    formula: Option<DcFormula>,
) -> Result<(), String> {
    if formula.is_some() && !matches!(formulation, crate::Problem::DcOpf | crate::Problem::DcPf) {
        return Err("branch_susceptance_formula is only supported for dcpf and dcopf".into());
    }
    Ok(())
}

pub(crate) fn validate_instance(
    instance: &powerio::DcOpfInstance,
    formula: Option<DcFormula>,
) -> Result<(), String> {
    if formula
        .is_some_and(|f| BranchSusceptanceFormula::from(f) != instance.branch_susceptance_formula())
    {
        return Err("branch_susceptance_formula conflicts with the typed DC OPF instance".into());
    }
    Ok(())
}

/// Materialize a retained DC choice once, before any save or re-solve.
#[cfg(feature = "sensitivity")]
pub(crate) fn apply_study_options(
    module: powerio::PioModule<powerio::PioValue>,
    formulation: crate::Problem,
    formula: Option<DcFormula>,
) -> Result<powerio::PioModule<powerio::PioValue>, String> {
    use powerio::{DcOpfInstance, PioValue};
    validate(formulation, formula)?;
    let Some(formula) = formula else {
        return Ok(module);
    };
    if formulation != crate::Problem::DcOpf {
        return Err("DC Study options require dcopf".into());
    }
    let value = match module.value() {
        PioValue::BalancedNetwork(net) => PioValue::DcOpfInstance(
            DcOpfInstance::from_network(net.clone())
                .map_err(|e| e.to_string())?
                .with_branch_susceptance_formula(formula.into()),
        ),
        PioValue::DcOpfInstance(instance) => {
            validate_instance(instance, Some(formula))?;
            module.value().clone()
        }
        _ => return Err("DC options require a balanced network or DC OPF instance Study".into()),
    };
    Ok(module.map_value(|_| value))
}
