//! Stateless settings and calculation HTTP adapters. No server-global preferences.
use super::*;
use tellegen::settings::{
    CalculationRequest, CalculationResult, ResolvedSettings, SettingsRequest,
};

pub(super) async fn describe() -> Json<serde_json::Value> {
    Json(tellegen::settings::describe())
}

pub(super) async fn resolve(
    Json(request): Json<SettingsRequest>,
) -> ApiResult<Json<ResolvedSettings>> {
    tellegen::settings::resolve(&request)
        .map(Json)
        .map_err(ApiError::bad_request)
}

/// Cancellation follows both request drop and timeout; the blocking worker owns
/// the permit until it actually finishes, even if its HTTP request goes away.
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

pub(super) async fn calculate(
    State(state): State<Arc<AppState>>,
    connect_info: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<CalculationRequest>,
) -> ApiResult<Json<CalculationResult>> {
    let client = expensive_client_key(&headers, &connect_info);
    state.check_expensive_rate_limit(ExpensiveEndpoint::Solve, &client)?;
    tellegen::settings::resolve(&request.settings).map_err(ApiError::bad_request)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let _cancel = CancelOnDrop(cancel.clone());
    let timeout = state.solver_timeout;
    tokio::time::timeout(timeout, async move {
        let permit = state
            .solver_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ApiError::service_unavailable("solver unavailable"))?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            tellegen::settings::calculate(&request, Some(cancel))
        })
        .await
        .map_err(|e| ApiError::internal(format!("calculation task failed: {e}")))?
        .map(Json)
        .map_err(ApiError::bad_request)
    })
    .await
    .map_err(|_| ApiError::service_unavailable("solve timed out"))?
}
