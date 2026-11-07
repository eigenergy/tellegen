//! Stateless settings and calculation HTTP adapters. No server-global preferences.
use super::*;
use axum::extract::FromRequest;
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

/// Largest accepted `/api/calculate` body; it carries a whole input module. The
/// largest served module, Texas7k, is about 37 MB. `deploy/Caddyfile` caps the
/// same path at the proxy.
pub(super) const CALCULATE_BODY_LIMIT: usize = 64 * 1024 * 1024;

fn too_large(detail: impl std::fmt::Display) -> ApiError {
    ApiError {
        status: StatusCode::PAYLOAD_TOO_LARGE,
        message: format!("calculation request rejected: {detail}"),
    }
}

/// Run one stateless calculation under the same limits as the other solver
/// endpoints, in this order:
///
/// 1. the per-client solve rate limit, before anything is read;
/// 2. a declared `Content-Length` over [`CALCULATE_BODY_LIMIT`] is refused with
///    413 before it can wait for, or hold, a solver permit;
/// 3. one deadline, as in `run_solve_limited`, bounds the wait for a permit, the
///    body read, the parse and the solve. Reading only once a permit is held
///    bounds the buffered request bodies to one per permit.
pub(super) async fn calculate(
    State(state): State<Arc<AppState>>,
    connect_info: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    request: Request,
) -> ApiResult<Json<CalculationResult>> {
    let client = expensive_client_key(&headers, &connect_info);
    state.check_expensive_rate_limit(ExpensiveEndpoint::Solve, &client)?;
    let declared = headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok()?.parse::<u64>().ok());
    if declared.is_some_and(|length| length > CALCULATE_BODY_LIMIT as u64) {
        return Err(too_large(format!(
            "body exceeds {CALCULATE_BODY_LIMIT} bytes"
        )));
    }
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
        // The route's DefaultBodyLimit applies here, so an undeclared or
        // understated body that runs over the limit is also a 413.
        let bytes = axum::body::Bytes::from_request(request, &())
            .await
            .map_err(|rejection| match rejection.status() {
                StatusCode::PAYLOAD_TOO_LARGE => too_large(rejection.body_text()),
                _ => ApiError::bad_request(format!(
                    "calculation request rejected: {}",
                    rejection.body_text()
                )),
            })?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let request: CalculationRequest = serde_json::from_slice(&bytes)
                .map_err(|e| ApiError::bad_request(format!("bad calculation request: {e}")))?;
            drop(bytes);
            tellegen::settings::calculate(&request, Some(cancel)).map_err(ApiError::bad_request)
        })
        .await
        .map_err(|e| ApiError::internal(format!("calculation task failed: {e}")))?
        .map(Json)
    })
    .await
    .map_err(|_| ApiError::service_unavailable("solve timed out"))?
}
