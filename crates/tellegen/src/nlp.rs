//! Shared POUNCE cancellation boundary for native formulations.
use pounce_nl::nl_reader::NlTnlp;
use pounce_rs::{
    BoundsInfo, Index, IpoptCq, IpoptData, IterStats, Linearity, MetaData, NlpInfo, Number,
    ScalingRequest, Solution, SparsityRequest, StartingPoint, TNLP,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
/// Transparent POUNCE model decorator that polls Tellegen's cancellation flag
/// from each intermediate callback made by the currently wired solve.
pub(crate) struct CancellableNlTnlp {
    pub(crate) inner: NlTnlp,
    cancel: Option<Arc<AtomicBool>>,
}

impl CancellableNlTnlp {
    pub(crate) fn new(inner: NlTnlp, cancel: Option<Arc<AtomicBool>>) -> Self {
        Self { inner, cancel }
    }
}

impl TNLP for CancellableNlTnlp {
    fn get_nlp_info(&mut self) -> Option<NlpInfo> {
        self.inner.get_nlp_info()
    }

    fn get_bounds_info(&mut self, bounds: BoundsInfo<'_>) -> bool {
        self.inner.get_bounds_info(bounds)
    }

    fn get_starting_point(&mut self, point: StartingPoint<'_>) -> bool {
        self.inner.get_starting_point(point)
    }

    fn eval_f(&mut self, x: &[Number], new_x: bool) -> Option<Number> {
        self.inner.eval_f(x, new_x)
    }

    fn eval_grad_f(&mut self, x: &[Number], new_x: bool, gradient: &mut [Number]) -> bool {
        self.inner.eval_grad_f(x, new_x, gradient)
    }

    fn eval_g(&mut self, x: &[Number], new_x: bool, constraints: &mut [Number]) -> bool {
        self.inner.eval_g(x, new_x, constraints)
    }

    fn eval_jac_g(
        &mut self,
        x: Option<&[Number]>,
        new_x: bool,
        request: SparsityRequest<'_>,
    ) -> bool {
        self.inner.eval_jac_g(x, new_x, request)
    }

    fn eval_h(
        &mut self,
        x: Option<&[Number]>,
        new_x: bool,
        objective_factor: Number,
        lambda: Option<&[Number]>,
        new_lambda: bool,
        request: SparsityRequest<'_>,
    ) -> bool {
        self.inner
            .eval_h(x, new_x, objective_factor, lambda, new_lambda, request)
    }

    fn finalize_solution(&mut self, solution: Solution<'_>, data: &IpoptData, cq: &IpoptCq) {
        self.inner.finalize_solution(solution, data, cq);
    }

    fn get_var_con_metadata(
        &mut self,
        variables: &mut MetaData,
        constraints: &mut MetaData,
    ) -> bool {
        self.inner.get_var_con_metadata(variables, constraints)
    }

    fn get_scaling_parameters(&mut self, request: ScalingRequest<'_>) -> bool {
        self.inner.get_scaling_parameters(request)
    }

    fn get_variables_linearity(&mut self, types: &mut [Linearity]) -> bool {
        self.inner.get_variables_linearity(types)
    }

    fn get_objective_variables_linearity(&mut self, types: &mut [Linearity]) -> bool {
        self.inner.get_objective_variables_linearity(types)
    }

    fn get_constraints_linearity(&mut self, types: &mut [Linearity]) -> bool {
        self.inner.get_constraints_linearity(types)
    }

    fn get_number_of_nonlinear_variables(&mut self) -> Index {
        self.inner.get_number_of_nonlinear_variables()
    }

    fn get_list_of_nonlinear_variables(&mut self, variables: &mut [Index]) -> bool {
        self.inner.get_list_of_nonlinear_variables(variables)
    }

    fn derivative_proofs(
        &mut self,
    ) -> pounce_rs::pounce_nlp::constant_derivatives::DerivativeProofs {
        self.inner.derivative_proofs()
    }

    fn intermediate_callback(
        &mut self,
        statistics: IterStats,
        data: &IpoptData,
        cq: &IpoptCq,
    ) -> bool {
        if self
            .cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return false;
        }
        self.inner.intermediate_callback(statistics, data, cq)
    }

    fn finalize_metadata(&mut self, variables: &MetaData, constraints: &MetaData) {
        self.inner.finalize_metadata(variables, constraints);
    }

    fn is_presolve_wrapper(&self) -> bool {
        self.inner.is_presolve_wrapper()
    }

    fn scaling_factors(&self) -> Option<Vec<Number>> {
        self.inner.scaling_factors()
    }

    fn presolve_infeasibility_proof(
        &self,
    ) -> Option<pounce_rs::pounce_nlp::tnlp::InfeasibilityProof> {
        self.inner.presolve_infeasibility_proof()
    }
}
