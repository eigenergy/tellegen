use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    convert::Infallible,
    env, fs,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Path as AxumPath, Query, Request, State},
    http::{header::CONTENT_TYPE, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    routing::{any, get},
    Json, Router,
};
use powerio::network::Network;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Semaphore};
use tokio_stream::wrappers::ReceiverStream;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use tellegen::geo::{complete_coords_for, spread_stacks, synthetic_layout, Coords};
use tellegen::{
    solve_prebuilt, solve_prebuilt_cancellable, DcNetwork, Iterations, SolveRequest, SolveResponse,
};
#[cfg(feature = "sensitivity")]
use tellegen::{ElementId, Mode, Operand, Parameter, Power, SensRequest, SensitivityMatrix};

const DEFAULT_PORT: u16 = 8000;
const DEFAULT_SOLVER_CONCURRENCY: usize = 2;
const DEFAULT_SOLVER_TIMEOUT_SECS: u64 = 30;
const DEFAULT_RATE_LIMIT_WINDOW_SECS: u64 = 10;
const DEFAULT_SOLVE_RATE_LIMIT_EVENTS: usize = 5;
const DEFAULT_SENSITIVITY_RATE_LIMIT_EVENTS: usize = 25;
const RATE_LIMIT_BUCKET_CAP: usize = 8192;

const CASE_SPECS: &[CaseSpec] = &[
    CaseSpec::aux(
        "case200",
        "ACTIVSg200 (Illinois)",
        "ACTIVSg200/case_ACTIVSg200.m",
        "ACTIVSg200/ACTIVSg200.aux",
    ),
    CaseSpec::aux(
        "case500",
        "ACTIVSg500 (South Carolina)",
        "ACTIVSg500/case_ACTIVSg500.m",
        "ACTIVSg500/ACTIVSg500.aux",
    ),
    CaseSpec::bus_csv(
        "case7000",
        "Texas7k (Texas)",
        "ACTIVSg7000/Texas7k_20210804.m",
        "ACTIVSg7000/Texas7k_lat_long.csv",
    ),
    CaseSpec::bus_csv_with_branch_geo(
        "cats",
        "CATS (California)",
        "CATS/CaliforniaTestSystem.m",
        "CATS/CATS_buses.csv",
        "CATS/CATS_lines.json",
    ),
];

const FALLBACK_SPECS: &[FallbackSpec] = &[
    FallbackSpec {
        id: "case200",
        name: "ACTIVSg200 (Illinois)",
        text: include_str!("../fixtures/pglib/pglib_opf_case200_activ.m"),
        bbox: (-91.4, 37.1, -87.6, 42.4),
    },
    FallbackSpec {
        id: "case500",
        name: "ACTIVSg500 (South Carolina)",
        text: include_str!("../fixtures/pglib/pglib_opf_case500_goc.m"),
        bbox: (-82.9, 33.3, -79.9, 35.0),
    },
];

#[derive(Clone, Copy)]
struct CaseSpec {
    id: &'static str,
    name: &'static str,
    casefile: &'static str,
    coords: CoordSpec,
    branch_geo: Option<&'static str>,
}

#[derive(Clone, Copy)]
enum CoordSpec {
    Aux(&'static str),
    BusCsv(&'static str),
}

impl CaseSpec {
    const fn aux(
        id: &'static str,
        name: &'static str,
        casefile: &'static str,
        auxfile: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            casefile,
            coords: CoordSpec::Aux(auxfile),
            branch_geo: None,
        }
    }

    const fn bus_csv(
        id: &'static str,
        name: &'static str,
        casefile: &'static str,
        csvfile: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            casefile,
            coords: CoordSpec::BusCsv(csvfile),
            branch_geo: None,
        }
    }

    const fn bus_csv_with_branch_geo(
        id: &'static str,
        name: &'static str,
        casefile: &'static str,
        csvfile: &'static str,
        branch_geo: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            casefile,
            coords: CoordSpec::BusCsv(csvfile),
            branch_geo: Some(branch_geo),
        }
    }

    fn coord_file(self) -> &'static str {
        match self.coords {
            CoordSpec::Aux(path) | CoordSpec::BusCsv(path) => path,
        }
    }
}

#[derive(Clone, Copy)]
struct FallbackSpec {
    id: &'static str,
    name: &'static str,
    text: &'static str,
    bbox: (f64, f64, f64, f64),
}

#[derive(Clone)]
pub struct AppState {
    cases: Arc<BTreeMap<String, Arc<CaseEntry>>>,
    solver_permits: Arc<Semaphore>,
    solver_timeout: Duration,
    expensive_rate_limits: RateLimitConfig,
    expensive_requests: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
    /// Gate on the compute endpoints (solve stream and sensitivities). Off by
    /// default so a public deploy serves data and the cached base solutions
    /// without exposing a path for on-demand solver load; `TELLEGEN_SERVER_COMPUTE`
    /// turns it on. The cases, network views, and `/solution` stay served.
    compute_enabled: bool,
}

#[derive(Clone, Copy)]
struct RateLimit {
    events: usize,
    window: Duration,
}

#[derive(Clone, Copy)]
struct RateLimitConfig {
    solve: RateLimit,
    sensitivity: RateLimit,
}

#[derive(Clone, Copy)]
enum ExpensiveEndpoint {
    Solve,
    Sensitivity,
}

impl ExpensiveEndpoint {
    fn name(self) -> &'static str {
        match self {
            ExpensiveEndpoint::Solve => "solve",
            ExpensiveEndpoint::Sensitivity => "sensitivity",
        }
    }

    fn limit(self, config: RateLimitConfig) -> RateLimit {
        match self {
            ExpensiveEndpoint::Solve => config.solve,
            ExpensiveEndpoint::Sensitivity => config.sensitivity,
        }
    }
}

struct CaseEntry {
    id: String,
    name: String,
    network: Network,
    network_json: String,
    /// The DC model built once at load. Solves clone this and perturb only the
    /// demand vector, so a demand drag never re-runs normalize-and-reindex.
    dc: Arc<DcNetwork>,
    view: NetworkPayload,
    base_solution: SolutionPayload,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseSummary {
    pub id: String,
    pub name: String,
    pub n_bus: usize,
    pub n_branch: usize,
    pub n_gen: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct HealthPayload {
    pub status: &'static str,
    pub cases: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetworkPayload {
    pub id: String,
    pub name: String,
    pub base_mva: f64,
    pub synthetic_coords: bool,
    pub buses: Vec<NetworkBus>,
    pub branches: Vec<NetworkBranch>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetworkBus {
    pub id: usize,
    pub lon: f64,
    pub lat: f64,
    pub demand_mw: f64,
    pub gen_mw: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetworkBranch {
    pub id: usize,
    pub from: usize,
    pub to: usize,
    pub rate_mw: f64,
    pub status: u8,
    pub path: Vec<[f64; 2]>,
}

/// The served DC value shapes (the HTTP/JSON contract). The engine returns the
/// formulation-agnostic `BusScalar` / `BranchFlow` / `GenDispatch`; the server maps
/// those to the DC-specific field names the frontend reads.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct LmpValue {
    pub bus: usize,
    pub usd_per_mwh: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct FlowValue {
    pub branch: usize,
    pub mw: f64,
    pub loading: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct DispatchValue {
    pub gen: usize,
    pub mw: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ScalarValue {
    pub bus: usize,
    pub value: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct SensitivityValue {
    pub bus: usize,
    pub value: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SolutionPayload {
    pub objective: f64,
    pub lmp: Vec<LmpValue>,
    pub va: Vec<ScalarValue>,
    pub w: Vec<ScalarValue>,
    pub flows: Vec<FlowValue>,
    pub dispatch: Vec<DispatchValue>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SensitivityPayload {
    pub case: String,
    pub operand: &'static str,
    pub parameter: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bus: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<usize>,
    pub units: String,
    pub values: Vec<SensitivityValue>,
}

#[derive(Debug, Deserialize)]
struct DemandQuery {
    d: Option<String>,
    sens: Option<usize>,
}

#[derive(Debug, Serialize)]
struct ErrorPayload {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn service_unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }

    fn too_many_requests(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: message.into(),
        }
    }

    fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorPayload {
                error: self.message,
            }),
        )
            .into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

impl AppState {
    pub fn load_from_env() -> Result<Self, String> {
        Ok(Self::load(data_dir(), allow_fallback())?.with_compute(server_compute_enabled()))
    }

    pub fn load(data_dir: PathBuf, allow_fallback: bool) -> Result<Self, String> {
        let staged_specs: Vec<_> = CASE_SPECS
            .iter()
            .copied()
            .filter(|spec| staged(&data_dir, spec))
            .collect();
        let mut cases = BTreeMap::new();

        // Serve whatever cases are staged rather than demanding the full set. A case
        // added to CASE_SPECS must not break a running deploy because its data has not
        // been staged yet: it simply appears once the data lands. A case whose files
        // are present but unparseable is skipped (logged), never fatal, so a bad data
        // drop cannot crash the server. The embedded fallback is used only when nothing
        // at all is staged. This keeps the served case set decoupled from any single
        // hardcoded list, so the deploy never fails over a missing or extra case.
        if !staged_specs.is_empty() {
            for spec in &staged_specs {
                match build_staged_entry(&data_dir, *spec) {
                    Ok(entry) => {
                        cases.insert(entry.id.clone(), Arc::new(entry));
                    }
                    Err(e) => {
                        tracing::error!(case = spec.id, "skipping case that failed to load: {e}")
                    }
                }
            }
            let missing: Vec<_> = CASE_SPECS
                .iter()
                .map(|s| s.id)
                .filter(|id| !cases.contains_key(*id))
                .collect();
            if !missing.is_empty() {
                tracing::warn!(
                    data_dir = %data_dir.display(),
                    "serving {} of {} cases; not loaded: {}",
                    cases.len(),
                    CASE_SPECS.len(),
                    missing.join(", ")
                );
            }
        } else if allow_fallback {
            tracing::warn!(
                data_dir = %data_dir.display(),
                "no staged case data; serving embedded pglib fallback cases"
            );
            for spec in FALLBACK_SPECS {
                let entry = build_fallback_entry(spec)?;
                cases.insert(entry.id.clone(), Arc::new(entry));
            }
        } else {
            return Err(format!(
                "no staged case data under {}. Run scripts/stage-data.sh or set TELLEGEN_ALLOW_FALLBACK=1 for the pglib dev fallback.",
                data_dir.display()
            ));
        }

        if cases.is_empty() {
            return Err(format!("no cases loaded from {}", data_dir.display()));
        }
        Ok(Self {
            cases: Arc::new(cases),
            solver_permits: Arc::new(Semaphore::new(solver_concurrency())),
            solver_timeout: solver_timeout(),
            expensive_rate_limits: rate_limit_config(),
            expensive_requests: Arc::new(Mutex::new(HashMap::new())),
            compute_enabled: false,
        })
    }

    /// Enable or disable the compute endpoints; `load` defaults to disabled.
    pub fn with_compute(mut self, enabled: bool) -> Self {
        self.compute_enabled = enabled;
        self
    }

    fn case(&self, id: &str) -> ApiResult<Arc<CaseEntry>> {
        self.cases
            .get(id)
            .cloned()
            .ok_or_else(|| ApiError::not_found(format!("unknown case {id}")))
    }

    fn case_ids(&self) -> Vec<String> {
        self.cases.keys().cloned().collect()
    }

    fn check_expensive_rate_limit(
        &self,
        endpoint: ExpensiveEndpoint,
        client: &str,
    ) -> ApiResult<()> {
        let limit = endpoint.limit(self.expensive_rate_limits);
        if limit.events == 0 {
            return Ok(());
        }

        let now = Instant::now();
        let key = format!("{}:{client}", endpoint.name());
        let mut buckets = self
            .expensive_requests
            .lock()
            .map_err(|_| ApiError::internal("rate limiter unavailable"))?;
        {
            let bucket = buckets.entry(key).or_default();
            prune_rate_bucket(bucket, now, limit.window);
            if bucket.len() >= limit.events {
                return Err(ApiError::too_many_requests(format!(
                    "{} rate limit exceeded",
                    endpoint.name()
                )));
            }
            bucket.push_back(now);
        }

        if buckets.len() > RATE_LIMIT_BUCKET_CAP {
            buckets.retain(|_, bucket| {
                prune_rate_bucket(bucket, now, limit.window);
                !bucket.is_empty()
            });
        }
        Ok(())
    }
}

pub async fn run_from_env() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    let state = Arc::new(AppState::load_from_env()?);
    let frontend = frontend_build_dir();
    let app = router(state, frontend);
    let addr: SocketAddr = format!(
        "{}:{}",
        env::var("TELLEGEN_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
        env::var("TELLEGEN_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT)
    )
    .parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "tellegen Rust server listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

pub fn router(state: Arc<AppState>, frontend_build: Option<PathBuf>) -> Router {
    // Every route that runs the solver on demand lives on this sub-router, so
    // the compute gate holds by construction: a new compute endpoint added here
    // is gated without remembering a per-handler check.
    let compute_routes = Router::new()
        .route(
            "/api/cases/{id}/sensitivity/lmp/d/{bus}",
            get(sensitivity_demand),
        )
        .route(
            "/api/cases/{id}/sensitivity/lmp/fmax/{branch}",
            get(sensitivity_line_limit),
        )
        .route("/api/cases/{id}/solve", get(solve_stream))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_compute,
        ));

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/compute", get(compute_status))
        .route("/api/cases", get(cases))
        .route("/api/cases/{id}/case", get(case_network_json))
        .route("/api/cases/{id}/network", get(network))
        .route("/api/cases/{id}/solution", get(solution))
        .merge(compute_routes)
        .route("/api/{*path}", any(api_not_found))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    if let Some(dir) = frontend_build.filter(|dir| dir.is_dir()) {
        let index = dir.join("index.html");
        let fallback = if dir.join("200.html").is_file() {
            dir.join("200.html")
        } else {
            index
        };
        app.fallback_service(
            ServeDir::new(dir)
                .append_index_html_on_directories(true)
                .precompressed_br()
                .precompressed_gzip()
                .fallback(
                    ServeFile::new(fallback)
                        .precompressed_br()
                        .precompressed_gzip(),
                ),
        )
    } else {
        app
    }
}

/// The compute gate for every route on the compute sub-router: 403 unless
/// `TELLEGEN_SERVER_COMPUTE` enabled the on-demand solver endpoints.
async fn require_compute(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    if state.compute_enabled {
        next.run(req).await
    } else {
        ApiError::forbidden("server compute is disabled").into_response()
    }
}

/// Whether the compute endpoints are enabled, so the frontend can pick honest
/// copy (and skip doomed requests) instead of inferring it from a 403.
async fn compute_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(serde_json::json!({ "enabled": state.compute_enabled }))
}

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let ids = state.case_ids();
    let status = if ids.is_empty() {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };
    (
        status,
        Json(HealthPayload {
            status: if ids.is_empty() { "degraded" } else { "ok" },
            cases: ids,
        }),
    )
}

async fn cases(State(state): State<Arc<AppState>>) -> Json<Vec<CaseSummary>> {
    Json(
        state
            .cases
            .values()
            .map(|entry| CaseSummary {
                id: entry.id.clone(),
                name: entry.name.clone(),
                n_bus: entry.network.buses.len(),
                n_branch: entry.network.branches.len(),
                n_gen: entry
                    .network
                    .generators
                    .iter()
                    .filter(|gen| gen.in_service)
                    .count(),
            })
            .collect(),
    )
}

async fn case_network_json(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<impl IntoResponse> {
    let entry = state.case(&id)?;
    Ok((
        [(CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        entry.network_json.clone(),
    ))
}

async fn network(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<NetworkPayload>> {
    Ok(Json(state.case(&id)?.view.clone()))
}

async fn solution(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<SolutionPayload>> {
    Ok(Json(state.case(&id)?.base_solution.clone()))
}

async fn api_not_found() -> ApiError {
    ApiError::not_found("unknown API route")
}

#[derive(Clone, Copy)]
enum SensitivityTarget {
    Demand { bus: usize },
    LineLimit { branch: usize },
}

async fn sensitivity_demand(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    connect_info: ConnectInfo<SocketAddr>,
    AxumPath((id, bus)): AxumPath<(String, usize)>,
    Query(query): Query<DemandQuery>,
) -> ApiResult<Json<SensitivityPayload>> {
    sensitivity(
        state,
        headers,
        connect_info,
        id,
        SensitivityTarget::Demand { bus },
        query,
    )
    .await
}

async fn sensitivity_line_limit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    connect_info: ConnectInfo<SocketAddr>,
    AxumPath((id, branch)): AxumPath<(String, usize)>,
    Query(query): Query<DemandQuery>,
) -> ApiResult<Json<SensitivityPayload>> {
    sensitivity(
        state,
        headers,
        connect_info,
        id,
        SensitivityTarget::LineLimit { branch },
        query,
    )
    .await
}

async fn sensitivity(
    state: Arc<AppState>,
    headers: HeaderMap,
    connect_info: ConnectInfo<SocketAddr>,
    id: String,
    target: SensitivityTarget,
    query: DemandQuery,
) -> ApiResult<Json<SensitivityPayload>> {
    let client = expensive_client_key(&headers, &connect_info);
    state.check_expensive_rate_limit(ExpensiveEndpoint::Sensitivity, &client)?;
    let entry = state.case(&id)?;
    match target {
        SensitivityTarget::Demand { bus } => {
            if !entry.network.buses.iter().any(|b| b.id.0 == bus) {
                return Err(ApiError::not_found(format!("unknown bus {bus}")));
            }
        }
        SensitivityTarget::LineLimit { branch } => {
            if !entry.dc.branch_ids.contains(&branch) {
                return Err(ApiError::not_found(format!("unknown branch {branch}")));
            }
        }
    }
    #[cfg(not(feature = "sensitivity"))]
    {
        let _ = query;
        return Err(ApiError::service_unavailable("sensitivity disabled"));
    }
    #[cfg(feature = "sensitivity")]
    {
        let deltas = parse_deltas(query.d.as_deref())?;
        validate_deltas(&entry, &deltas)?;
        let request = build_request(&entry, deltas, Some(target));
        let id_for_task = entry.id.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let output = run_solve_limited(state, entry, request, cancel).await?;
        let Some(m) = output.sensitivities.first() else {
            return Err(ApiError::not_found("unknown sensitivity target"));
        };
        Ok(Json(sensitivity_payload(&id_for_task, m)))
    }
}

async fn solve_stream(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    connect_info: ConnectInfo<SocketAddr>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<DemandQuery>,
) -> ApiResult<Sse<ReceiverStream<Result<Event, Infallible>>>> {
    let client = expensive_client_key(&headers, &connect_info);
    state.check_expensive_rate_limit(ExpensiveEndpoint::Solve, &client)?;
    let entry = state.case(&id)?;
    if let Some(bus) = query.sens {
        if !entry.network.buses.iter().any(|b| b.id.0 == bus) {
            return Err(ApiError::not_found(format!("unknown bus {bus}")));
        }
    }
    #[cfg(not(feature = "sensitivity"))]
    if query.sens.is_some() {
        return Err(ApiError::service_unavailable("sensitivity disabled"));
    }

    let deltas = parse_deltas(query.d.as_deref())?;
    validate_deltas(&entry, &deltas)?;
    let request = build_request(
        &entry,
        deltas,
        query.sens.map(|bus| SensitivityTarget::Demand { bus }),
    );
    let (tx, rx) = mpsc::channel(8);
    let id_for_task = entry.id.clone();
    tokio::spawn(async move {
        send_event(
            &tx,
            "status",
            &serde_json::json!({ "phase": "solving", "case": id_for_task.as_str() }),
        )
        .await;
        let start = Instant::now();
        let cancel = Arc::new(AtomicBool::new(false));
        let result = tokio::select! {
            biased;
            // Client hung up: cancel the in-flight solve and drop the stream so
            // its solver permit is released instead of pinned to convergence.
            _ = tx.closed() => {
                cancel.store(true, Ordering::Relaxed);
                return;
            }
            r = run_solve_limited(state, entry, request, cancel.clone()) => r,
        };
        match result {
            Ok(output) => {
                let solve_ms = start.elapsed().as_secs_f64() * 1000.0;
                let solution = solution_payload(&output);
                let iterations = match &output.iterations {
                    Some(Iterations::Ipm(trace)) => trace.as_slice(),
                    _ => &[][..],
                };
                send_event(
                    &tx,
                    "solution",
                    &serde_json::json!({
                        "case": id_for_task.as_str(),
                        "solve_ms": (solve_ms * 10.0).round() / 10.0,
                        "objective": solution.objective,
                        "lmp": solution.lmp,
                        "va": solution.va,
                        "w": solution.w,
                        "flows": solution.flows,
                        "dispatch": solution.dispatch,
                        "iterations": iterations,
                    }),
                )
                .await;
                #[cfg(feature = "sensitivity")]
                if let Some(m) = output.sensitivities.first() {
                    send_event(&tx, "sensitivity", &sensitivity_payload(&id_for_task, m)).await;
                }
                send_event(&tx, "done", &serde_json::json!({ "ok": true })).await;
            }
            Err(error) => {
                send_event(&tx, "fail", &serde_json::json!({ "error": error.message })).await;
            }
        }
    });
    Ok(Sse::new(ReceiverStream::new(rx)))
}

/// Build a DC OPF [`SolveRequest`] for the cached case: the operating-point demand
/// deltas, plus a single Price sensitivity column when a target is given.
fn build_request(
    entry: &CaseEntry,
    deltas: HashMap<usize, f64>,
    target: Option<SensitivityTarget>,
) -> SolveRequest {
    let mut request = SolveRequest::default();
    request.edits.deltas = deltas
        .into_iter()
        .map(|(bus, mw)| (bus as i64, mw))
        .collect();
    #[cfg(feature = "sensitivity")]
    if let Some((parameter, idx)) = target.and_then(|target| match target {
        SensitivityTarget::Demand { bus } => entry
            .dc
            .bus_ids
            .iter()
            .position(|&id| id == bus)
            .map(|idx| (Parameter::Demand(Power::Active), idx)),
        SensitivityTarget::LineLimit { branch } => entry
            .dc
            .branch_ids
            .iter()
            .position(|&id| id == branch)
            .map(|idx| (Parameter::LineLimit, idx)),
    }) {
        request.sensitivities = vec![SensRequest {
            operand: Operand::Price(Power::Active),
            parameter,
            indices: Some(vec![idx]),
            mode: Mode::Auto,
        }];
    }
    #[cfg(not(feature = "sensitivity"))]
    let _ = (entry, target);
    request
}

async fn run_solve_limited(
    state: Arc<AppState>,
    entry: Arc<CaseEntry>,
    request: SolveRequest,
    cancel: Arc<AtomicBool>,
) -> ApiResult<SolveResponse> {
    let timeout = state.solver_timeout;
    let task_cancel = cancel.clone();
    // One deadline bounds the whole operation: the wait for a solver permit and
    // the solve itself. Acquiring inside the timeout means a saturated pool
    // returns "solve timed out" instead of queueing unbounded. On timeout we
    // flip the cancel flag so an in-flight solve stops at its next interior-point
    // iteration and releases its permit, instead of running to convergence for a
    // request that already gave up (the cancel is observed within one iteration,
    // sub-millisecond for the served cases).
    let result = tokio::time::timeout(timeout, async move {
        let permit = state
            .solver_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ApiError::service_unavailable("solver unavailable"))?;
        let dc = entry.dc.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            solve_prebuilt_cancellable(&dc, &request, Some(task_cancel))
        })
        .await
        .map_err(|e| ApiError::internal(format!("solve task failed: {e}")))?
        .map_err(ApiError::internal)
    })
    .await;
    match result {
        Ok(r) => r,
        Err(_) => {
            cancel.store(true, Ordering::Relaxed);
            Err(ApiError::service_unavailable("solve timed out"))
        }
    }
}

fn build_staged_entry(data_dir: &Path, spec: CaseSpec) -> Result<CaseEntry, String> {
    let case_path = data_dir.join(spec.casefile);
    let case = powerio::format::parse_file(&case_path, Some("m"))
        .map_err(|e| format!("{}: {e}", case_path.display()))?
        .network;
    let coords = match spec.coords {
        CoordSpec::Aux(auxfile) => {
            let aux_path = data_dir.join(auxfile);
            let aux = powerio::format::parse_file(&aux_path, Some("aux"))
                .map_err(|e| format!("{}: {e}", aux_path.display()))?
                .network;
            complete_coords_for(&case, &aux, auxfile)?
        }
        CoordSpec::BusCsv(csvfile) => load_bus_csv_coords(&data_dir.join(csvfile), &case)?,
    };
    let branch_paths = spec
        .branch_geo
        .and_then(|file| {
            let path = data_dir.join(file);
            path.is_file().then(|| load_branch_paths(&path))
        })
        .transpose()?;
    build_entry(spec.id, spec.name, case, coords, branch_paths, false)
}

fn load_bus_csv_coords(path: &Path, case: &Network) -> Result<Coords, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| format!("{}: empty coordinate CSV", path.display()))?;
    let headers: Vec<_> = header.split(',').map(str::trim).collect();
    let col = |names: &[&str]| {
        names
            .iter()
            .find_map(|name| headers.iter().position(|h| h.eq_ignore_ascii_case(name)))
            .ok_or_else(|| {
                format!(
                    "{}: missing one of {} columns",
                    path.display(),
                    names.join(", ")
                )
            })
    };
    let bus_i = col(&["bus_i", "Bus_ID", "bus"])?;
    let lat_i = col(&["Lat", "lat", "latitude"])?;
    let lon_i = col(&["Lon", "lng", "lon", "longitude"])?;
    let needed = bus_i.max(lat_i).max(lon_i);
    let mut coords = Coords::new();
    for (idx, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Vec<_> = line.split(',').map(str::trim).collect();
        if row.len() <= needed {
            return Err(format!(
                "{}:{}: expected at least {} columns",
                path.display(),
                idx + 2,
                needed + 1
            ));
        }
        let bus = row[bus_i]
            .parse::<usize>()
            .map_err(|e| format!("{}:{}: bad bus_i: {e}", path.display(), idx + 2))?;
        let lat = row[lat_i]
            .parse::<f64>()
            .map_err(|e| format!("{}:{}: bad Lat: {e}", path.display(), idx + 2))?;
        let lon = row[lon_i]
            .parse::<f64>()
            .map_err(|e| format!("{}:{}: bad Lon: {e}", path.display(), idx + 2))?;
        if lat.is_finite() && lon.is_finite() {
            coords.insert(bus, (lon, lat));
        }
    }
    spread_stacks(&mut coords);
    for bus in &case.buses {
        if !coords.contains_key(&bus.id.0) {
            return Err(format!(
                "{}: missing coordinates for bus {}",
                path.display(),
                bus.id.0
            ));
        }
    }
    Ok(coords)
}

/// Branch geometry with one stored copy per feature: `index` maps every id or
/// endpoint key onto the shared path, so a 10k-feature GeoJSON is held once
/// instead of once per key.
#[derive(Default)]
struct BranchPaths {
    paths: Vec<Vec<[f64; 2]>>,
    index: HashMap<BranchPathKey, usize>,
}

impl BranchPaths {
    fn insert(&mut self, keys: Vec<BranchPathKey>, path: Vec<[f64; 2]>) {
        if keys.is_empty() {
            return;
        }
        let slot = self.paths.len();
        for key in keys {
            self.index.insert(key, slot);
        }
        self.paths.push(path);
    }

    fn get(&self, key: &BranchPathKey) -> Option<&Vec<[f64; 2]>> {
        self.index.get(key).map(|&i| &self.paths[i])
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum BranchPathKey {
    Id(usize),
    Edge(usize, usize),
}

fn load_branch_paths(path: &Path) -> Result<BranchPaths, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    load_branch_paths_from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn load_branch_paths_from_str(text: &str) -> Result<BranchPaths, String> {
    let data: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let features = data
        .get("features")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "expected GeoJSON FeatureCollection with features".to_string())?;
    let mut paths = BranchPaths::default();
    for feature in features {
        add_branch_feature_paths(feature, &mut paths);
    }
    Ok(paths)
}

fn add_branch_feature_paths(feature: &serde_json::Value, paths: &mut BranchPaths) {
    let Some(geometry) = feature.get("geometry").and_then(|v| v.as_object()) else {
        return;
    };
    if geometry.get("type").and_then(|v| v.as_str()) != Some("LineString") {
        return;
    }
    let path = coord_path(geometry.get("coordinates"));
    if path.len() < 2 {
        return;
    }
    let props = feature
        .get("properties")
        .and_then(|v| v.as_object())
        .or_else(|| feature.as_object());
    let Some(props) = props else {
        return;
    };
    // A bare `id` is not accepted as a branch number: RFC 7946 allows a
    // feature counter under that name, and matching it would silently assign
    // another line's geometry whenever the counter order differs from the
    // branch row order.
    let mut keys = Vec::new();
    if let Some(id) = find_json_number(props, &["branch", "branch_id", "branch number", "cats_id"])
    {
        keys.push(BranchPathKey::Id(id));
    }
    if let (Some(from), Some(to)) = (
        find_json_number(props, &["f_bus", "from", "from_bus"]),
        find_json_number(props, &["t_bus", "to", "to_bus"]),
    ) {
        keys.push(BranchPathKey::Edge(from, to));
    }
    paths.insert(keys, path);
}

fn coord_path(value: Option<&serde_json::Value>) -> Vec<[f64; 2]> {
    value
        .and_then(|v| v.as_array())
        .map(|coords| {
            coords
                .iter()
                .filter_map(|coord| {
                    let pair = coord.as_array()?;
                    let lon = pair.first()?.as_f64()?;
                    let lat = pair.get(1)?.as_f64()?;
                    valid_coord(lon, lat).then_some([lon, lat])
                })
                .collect()
        })
        .unwrap_or_default()
}

fn find_json_number(
    props: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<usize> {
    let wanted: HashSet<String> = names.iter().map(|name| normalize_key(name)).collect();
    props.iter().find_map(|(key, value)| {
        if !wanted.contains(&normalize_key(key)) {
            return None;
        }
        json_number(value)
    })
}

fn json_number(value: &serde_json::Value) -> Option<usize> {
    if let Some(n) = value.as_u64() {
        return usize::try_from(n).ok();
    }
    if let Some(n) = value.as_i64() {
        return usize::try_from(n).ok();
    }
    if let Some(s) = value.as_str() {
        let clean = s.trim().trim_matches(['"', '\'']);
        return clean.parse::<usize>().ok();
    }
    None
}

fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn valid_coord(lon: f64, lat: f64) -> bool {
    lon.is_finite() && lat.is_finite() && lon.abs() <= 180.0 && lat.abs() <= 90.0
}

fn build_fallback_entry(spec: &FallbackSpec) -> Result<CaseEntry, String> {
    let case = powerio::parse_str(spec.text, "m")
        .map_err(|e| format!("{} fallback parse failed: {e}", spec.id))?
        .network;
    let coords = synthetic_layout(&case, spec.bbox);
    build_entry(spec.id, spec.name, case, coords, None, true)
}

fn build_entry(
    id: &str,
    name: &str,
    network: Network,
    coords: Coords,
    branch_paths: Option<BranchPaths>,
    synthetic_coords: bool,
) -> Result<CaseEntry, String> {
    let network_json = network.to_json().map_err(|e| e.to_string())?;
    let dc = Arc::new(DcNetwork::from_network(&network)?);
    let base = solve_prebuilt(&dc, &SolveRequest::default())?;
    let view = network_payload(
        id,
        name,
        &network,
        &coords,
        branch_paths.as_ref(),
        synthetic_coords,
    )?;
    Ok(CaseEntry {
        id: id.into(),
        name: name.into(),
        network,
        network_json,
        dc,
        view,
        base_solution: solution_payload(&base),
    })
}

fn network_payload(
    id: &str,
    name: &str,
    net: &Network,
    coords: &Coords,
    branch_paths: Option<&BranchPaths>,
    synthetic_coords: bool,
) -> Result<NetworkPayload, String> {
    let mut demand = BTreeMap::<usize, f64>::new();
    for load in net.loads.iter().filter(|load| load.in_service) {
        *demand.entry(load.bus.0).or_default() += load.p;
    }
    let mut generation = BTreeMap::<usize, f64>::new();
    for gen in net.generators.iter().filter(|gen| gen.in_service) {
        *generation.entry(gen.bus.0).or_default() += gen.pmax;
    }
    let buses = net
        .buses
        .iter()
        .map(|bus| {
            let &(lon, lat) = coords
                .get(&bus.id.0)
                .ok_or_else(|| format!("{}: missing coordinates for bus {}", id, bus.id.0))?;
            Ok(NetworkBus {
                id: bus.id.0,
                lon,
                lat,
                demand_mw: demand.get(&bus.id.0).copied().unwrap_or(0.0),
                gen_mw: generation.get(&bus.id.0).copied().unwrap_or(0.0),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let branches = net
        .branches
        .iter()
        .enumerate()
        .map(|(i, br)| {
            let &(from_lon, from_lat) = coords.get(&br.from.0).ok_or_else(|| {
                format!(
                    "{}: missing coordinates for branch from bus {}",
                    id, br.from.0
                )
            })?;
            let &(to_lon, to_lat) = coords.get(&br.to.0).ok_or_else(|| {
                format!("{}: missing coordinates for branch to bus {}", id, br.to.0)
            })?;
            Ok(NetworkBranch {
                id: i + 1,
                from: br.from.0,
                to: br.to.0,
                rate_mw: br.rate_a,
                status: br.in_service as u8,
                path: branch_paths
                    .and_then(|paths| {
                        paths
                            .get(&BranchPathKey::Id(i + 1))
                            .or_else(|| paths.get(&BranchPathKey::Edge(br.from.0, br.to.0)))
                            .or_else(|| paths.get(&BranchPathKey::Edge(br.to.0, br.from.0)))
                    })
                    .cloned()
                    .unwrap_or_else(|| vec![[from_lon, from_lat], [to_lon, to_lat]]),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(NetworkPayload {
        id: id.into(),
        name: name.into(),
        base_mva: net.base_mva,
        synthetic_coords,
        buses,
        branches,
    })
}

fn solution_payload(resp: &SolveResponse) -> SolutionPayload {
    SolutionPayload {
        objective: resp.objective.unwrap_or(0.0),
        lmp: resp
            .lmp
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|s| LmpValue {
                bus: s.bus,
                usd_per_mwh: s.value,
            })
            .collect(),
        va: scalar_values(resp.va.as_deref()),
        w: scalar_values(resp.w.as_deref()),
        flows: resp
            .flows
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|f| FlowValue {
                branch: f.branch,
                mw: f.pf,
                loading: f.loading,
            })
            .collect(),
        dispatch: resp
            .dispatch
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|g| DispatchValue {
                gen: g.gen,
                mw: g.pg,
            })
            .collect(),
    }
}

fn scalar_values(values: Option<&[tellegen::BusScalar]>) -> Vec<ScalarValue> {
    values
        .unwrap_or_default()
        .iter()
        .map(|s| ScalarValue {
            bus: s.bus,
            value: s.value,
        })
        .collect()
}

/// The dLMP/dparameter column from the requested Price cell. Rows are buses, and
/// the single column names the bus or branch parameter through matrix metadata.
#[cfg(feature = "sensitivity")]
fn sensitivity_payload(case: &str, m: &SensitivityMatrix) -> SensitivityPayload {
    let (parameter, bus, branch) =
        m.cols
            .first()
            .map_or(("unknown", None, None), |c| match c.element {
                ElementId::Bus(b) => ("d", Some(b), None),
                ElementId::Branch(b) => ("fmax", None, Some(b)),
                _ => ("unknown", None, None),
            });
    let values = m
        .rows
        .iter()
        .zip(&m.values)
        .map(|(row, vals)| SensitivityValue {
            bus: match row.element {
                ElementId::Bus(b) => b,
                _ => 0,
            },
            value: vals.first().copied().unwrap_or(0.0),
        })
        .collect();
    SensitivityPayload {
        case: case.into(),
        operand: "lmp",
        parameter,
        bus,
        branch,
        units: m.units.clone(),
        values,
    }
}

fn parse_deltas(input: Option<&str>) -> ApiResult<HashMap<usize, f64>> {
    let mut deltas = HashMap::new();
    let mut seen = HashSet::new();
    for raw in input.unwrap_or("").split(',') {
        let part = raw.trim();
        if part.is_empty() {
            continue;
        }
        let (bus, mw) = part
            .split_once(':')
            .ok_or_else(|| ApiError::bad_request(format!("invalid demand delta {part:?}")))?;
        let bus = bus
            .trim()
            .parse::<usize>()
            .map_err(|_| ApiError::bad_request(format!("invalid demand delta bus {bus:?}")))?;
        if bus == 0 {
            return Err(ApiError::bad_request("demand delta bus must be positive"));
        }
        if !seen.insert(bus) {
            return Err(ApiError::bad_request(format!(
                "duplicate demand delta for bus {bus}"
            )));
        }
        let mw = mw
            .trim()
            .parse::<f64>()
            .map_err(|_| ApiError::bad_request(format!("invalid demand delta MW {mw:?}")))?;
        if !mw.is_finite() {
            return Err(ApiError::bad_request(format!(
                "demand delta for bus {bus} must be finite"
            )));
        }
        deltas.insert(bus, mw);
    }
    Ok(deltas)
}

fn validate_deltas(entry: &CaseEntry, deltas: &HashMap<usize, f64>) -> ApiResult<()> {
    for (&bus, &delta) in deltas {
        let base = entry
            .view
            .buses
            .iter()
            .find(|b| b.id == bus)
            .ok_or_else(|| ApiError::bad_request(format!("unknown demand delta bus {bus}")))?
            .demand_mw;
        // A bus in the full network but excluded from the DC model (an isolated
        // MATPOWER type 4 bus) would fail deep inside the engine as "unknown" and
        // surface as a 500; reject it here with an accurate message instead.
        if !entry.dc.bus_ids.contains(&bus) {
            return Err(ApiError::bad_request(format!(
                "demand delta bus {bus} is isolated and excluded from the model"
            )));
        }
        if base + delta < -1e-9 {
            return Err(ApiError::bad_request(format!(
                "demand delta for bus {bus} would make demand negative"
            )));
        }
    }
    Ok(())
}

async fn send_event<T: Serialize>(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    event: &str,
    payload: &T,
) {
    if let Ok(data) = serde_json::to_string(payload) {
        let _ = tx.send(Ok(Event::default().event(event).data(data))).await;
    }
}

fn staged(data_dir: &Path, spec: &CaseSpec) -> bool {
    data_dir.join(spec.casefile).is_file() && data_dir.join(spec.coord_file()).is_file()
}

fn data_dir() -> PathBuf {
    env::var_os("TELLEGEN_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            first_existing(default_data_dirs()).unwrap_or_else(|| PathBuf::from("data"))
        })
}

fn frontend_build_dir() -> Option<PathBuf> {
    env::var_os("TELLEGEN_FRONTEND_BUILD")
        .map(PathBuf::from)
        .or_else(|| first_existing(default_frontend_dirs()))
}

fn first_existing(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|path| path.exists())
}

fn default_data_dirs() -> Vec<PathBuf> {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    vec![
        cwd.join("data"),
        cwd.join("../data"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
    ]
}

fn default_frontend_dirs() -> Vec<PathBuf> {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    vec![
        cwd.join("apps/web/build"),
        cwd.join("../apps/web/build"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/build"),
    ]
}

/// A boolean env var: `1`/`true`/`yes`/`on` (case insensitive) enables.
fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn allow_fallback() -> bool {
    env_flag("TELLEGEN_ALLOW_FALLBACK")
}

fn server_compute_enabled() -> bool {
    env_flag("TELLEGEN_SERVER_COMPUTE")
}

fn solver_concurrency() -> usize {
    env::var("TELLEGEN_SOLVER_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_SOLVER_CONCURRENCY)
}

fn solver_timeout() -> Duration {
    Duration::from_secs(
        env::var("TELLEGEN_SOLVER_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_SOLVER_TIMEOUT_SECS),
    )
}

fn rate_limit_config() -> RateLimitConfig {
    let window = Duration::from_secs(
        env::var("TELLEGEN_RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_RATE_LIMIT_WINDOW_SECS),
    );
    RateLimitConfig {
        solve: RateLimit {
            events: env_rate_limit_events(
                "TELLEGEN_SOLVE_RATE_LIMIT_EVENTS",
                DEFAULT_SOLVE_RATE_LIMIT_EVENTS,
            ),
            window,
        },
        sensitivity: RateLimit {
            events: env_rate_limit_events(
                "TELLEGEN_SENSITIVITY_RATE_LIMIT_EVENTS",
                DEFAULT_SENSITIVITY_RATE_LIMIT_EVENTS,
            ),
            window,
        },
    }
}

fn env_rate_limit_events(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(default)
}

fn expensive_client_key(headers: &HeaderMap, connect_info: &ConnectInfo<SocketAddr>) -> String {
    let peer = connect_info.0.ip();
    if trusted_proxy_peer(peer) {
        // Take the rightmost x-forwarded-for entry: it is the one appended by the
        // nearest proxy. The leftmost entries are client-supplied under proxies that
        // append rather than replace, which would let a client mint a fresh rate
        // limit key per request.
        if let Some(key) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').rev().find_map(clean_client_key))
        {
            return key;
        }
        if let Some(key) = headers
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(clean_client_key)
        {
            return key;
        }
    }
    peer.to_string()
}

fn trusted_proxy_peer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private(),
        IpAddr::V6(ip) => {
            let first = ip.segments()[0];
            ip.is_loopback() || (first & 0xfe00) == 0xfc00
        }
    }
}

fn clean_client_key(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(128).collect())
    }
}

fn prune_rate_bucket(bucket: &mut VecDeque<Instant>, now: Instant, window: Duration) {
    while bucket
        .front()
        .and_then(|seen| now.checked_duration_since(*seen))
        .is_some_and(|age| age >= window)
    {
        bucket.pop_front();
    }
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "tellegen_server=info,tower_http=warn".into());
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::HeaderMap;
    use std::fs;
    use std::sync::{
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
        OnceLock,
    };
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::ServiceExt;

    fn fallback_state() -> Arc<AppState> {
        static STATE: OnceLock<Arc<AppState>> = OnceLock::new();
        Arc::clone(STATE.get_or_init(|| {
            Arc::new(
                AppState::load(PathBuf::from("/definitely/no/tellegen/data"), true)
                    .unwrap()
                    .with_compute(true),
            )
        }))
    }

    /// The default state: compute disabled, as a public deploy ships it.
    fn compute_disabled_state() -> Arc<AppState> {
        static STATE: OnceLock<Arc<AppState>> = OnceLock::new();
        Arc::clone(STATE.get_or_init(|| {
            Arc::new(AppState::load(PathBuf::from("/definitely/no/tellegen/data"), true).unwrap())
        }))
    }

    static NEXT_CLIENT: AtomicUsize = AtomicUsize::new(1);

    fn next_client() -> String {
        format!(
            "203.0.113.{}",
            NEXT_CLIENT.fetch_add(1, AtomicOrdering::Relaxed)
        )
    }

    async fn get_raw(path: &str) -> (StatusCode, HeaderMap, String) {
        get_raw_with_state(fallback_state(), path, &next_client()).await
    }

    async fn get_raw_from(path: &str, client: &str) -> (StatusCode, HeaderMap, String) {
        get_raw_with_state(fallback_state(), path, client).await
    }

    async fn get_raw_with_state(
        state: Arc<AppState>,
        path: &str,
        client: &str,
    ) -> (StatusCode, HeaderMap, String) {
        let res = router(state, None)
            .oneshot(
                axum::http::Request::builder()
                    .uri(path)
                    .header("x-forwarded-for", client)
                    .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, headers, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn get(path: &str) -> (StatusCode, serde_json::Value) {
        let (status, _headers, body) = get_raw(path).await;
        (status, serde_json::from_str(&body).unwrap())
    }

    async fn get_from(path: &str, client: &str) -> (StatusCode, serde_json::Value) {
        let (status, _headers, body) = get_raw_from(path, client).await;
        (status, serde_json::from_str(&body).unwrap())
    }

    async fn static_get(path: &str, dir: PathBuf) -> (StatusCode, String) {
        let res = router(fallback_state(), Some(dir))
            .oneshot(
                axum::http::Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn health_lists_fallback_cases() {
        let (status, body) = get("/api/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["cases"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn case_endpoints_have_expected_shapes() {
        let (status, cases) = get("/api/cases").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(cases.as_array().unwrap().len(), 2);
        assert_eq!(cases[0]["n_gen"], 38);

        let (status, network) = get("/api/cases/case200/network").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(network["id"], "case200");
        assert!(network["synthetic_coords"].as_bool().unwrap());
        assert!(network["buses"].as_array().unwrap().len() >= 200);

        let (status, solution) = get("/api/cases/case200/solution").await;
        assert_eq!(status, StatusCode::OK);
        assert!(solution["objective"].as_f64().unwrap().is_finite());
        assert!(solution["lmp"].as_array().unwrap().len() >= 200);
    }

    #[test]
    fn branch_geojson_paths_match_by_cats_id_and_endpoint_buses() {
        let paths = load_branch_paths_from_str(
            r#"{
              "type": "FeatureCollection",
              "features": [
                {
                  "type": "Feature",
                  "properties": { "CATS_ID": "7", "f_bus": 101, "t_bus": 202 },
                  "geometry": {
                    "type": "LineString",
                    "coordinates": [[-122.0, 37.0], [-121.5, 37.2], [-121.0, 37.4]]
                  }
                }
              ]
            }"#,
        )
        .unwrap();
        let expected = vec![[-122.0, 37.0], [-121.5, 37.2], [-121.0, 37.4]];
        assert_eq!(paths.get(&BranchPathKey::Id(7)), Some(&expected));
        assert_eq!(paths.get(&BranchPathKey::Edge(101, 202)), Some(&expected));
    }

    #[tokio::test]
    async fn static_fallback_serves_200_html_without_index() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!("tellegen-static-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("200.html"),
            "<!doctype html><title>tellegen</title>",
        )
        .unwrap();

        let (status, body) = static_get("/", dir.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("tellegen"));

        let (status, body) = static_get("/map/path", dir.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("tellegen"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(feature = "sensitivity")]
    #[tokio::test]
    async fn sensitivity_returns_payload() {
        let (status, body) = get("/api/cases/case200/sensitivity/lmp/d/1?d=1:5").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["case"], "case200");
        assert_eq!(body["bus"], 1);
        assert_eq!(body["parameter"], "d");
        assert!(body["values"].as_array().unwrap().len() >= 200);
    }

    #[cfg(feature = "sensitivity")]
    #[tokio::test]
    async fn line_limit_sensitivity_returns_payload() {
        let (status, body) = get("/api/cases/case200/sensitivity/lmp/fmax/1").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["case"], "case200");
        assert_eq!(body["parameter"], "fmax");
        assert_eq!(body["branch"], 1);
        assert!(body.get("bus").is_none());
        assert!(body["values"].as_array().unwrap().len() >= 200);
    }

    async fn get_with_state(state: Arc<AppState>, path: &str) -> (StatusCode, serde_json::Value) {
        let (status, _headers, body) = get_raw_with_state(state, path, &next_client()).await;
        (status, serde_json::from_str(&body).unwrap())
    }

    #[tokio::test]
    async fn compute_endpoints_are_disabled_by_default() {
        for path in [
            "/api/cases/case200/sensitivity/lmp/d/1",
            "/api/cases/case200/sensitivity/lmp/fmax/1",
            "/api/cases/case200/solve",
        ] {
            let (status, body) = get_with_state(compute_disabled_state(), path).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
            assert_eq!(body["error"], "server compute is disabled", "{path}");
        }
        // Data endpoints stay served, including the cached base solution, and
        // /api/compute reports the gate to the frontend.
        let (status, _) =
            get_with_state(compute_disabled_state(), "/api/cases/case200/solution").await;
        assert_eq!(status, StatusCode::OK);
        let (status, body) = get_with_state(compute_disabled_state(), "/api/compute").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["enabled"], false);
        let (status, body) = get_with_state(fallback_state(), "/api/compute").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["enabled"], true);
    }

    #[cfg(not(feature = "sensitivity"))]
    #[tokio::test]
    async fn sensitivity_returns_unavailable_without_feature() {
        let (status, body) = get("/api/cases/case200/sensitivity/lmp/d/1").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body["error"]
            .as_str()
            .unwrap()
            .contains("sensitivity disabled"));
    }

    #[tokio::test]
    async fn demand_delta_validation_rejects_bad_requests() {
        for path in [
            "/api/cases/case200/solve?d=junk",
            "/api/cases/case200/solve?d=1:notnum",
            "/api/cases/case200/solve?d=1:NaN",
            "/api/cases/case200/solve?d=1:5,1:6",
            "/api/cases/case200/solve?d=999999:1",
            "/api/cases/case200/solve?d=1:-999999",
        ] {
            let (status, body) = get(path).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {body}");
            assert!(body["error"].as_str().unwrap().len() > 5);
        }
    }

    #[tokio::test]
    async fn missing_case_and_bus_return_json_404() {
        let (status, body) = get("/api/cases/nope/network").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("unknown case"));

        let (status, body) = get("/api/cases/case200/sensitivity/lmp/d/999999").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("unknown bus"));

        let (status, body) = get("/api/unknown").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"]
            .as_str()
            .unwrap()
            .contains("unknown API route"));
    }

    #[tokio::test]
    async fn solve_route_rate_limits_same_client_before_case_lookup() {
        let client = "198.51.100.10";
        for _ in 0..DEFAULT_SOLVE_RATE_LIMIT_EVENTS {
            let (status, body) = get_from("/api/cases/nope/solve", client).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
            assert!(body["error"].as_str().unwrap().contains("unknown case"));
        }

        let (status, body) = get_from("/api/cases/nope/solve", client).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert!(body["error"].as_str().unwrap().contains("rate limit"));
    }

    #[tokio::test]
    async fn rate_limit_keys_on_rightmost_forwarded_entry() {
        // Proxies that append leave client-supplied entries on the left of
        // x-forwarded-for; only the rightmost entry comes from the proxy itself.
        // Varying left entries must not mint fresh rate limit keys.
        for i in 0..DEFAULT_SOLVE_RATE_LIMIT_EVENTS {
            let spoofed = format!("203.0.113.{i}, 198.51.100.77");
            let (status, _) = get_from("/api/cases/nope/solve", &spoofed).await;
            assert_eq!(status, StatusCode::NOT_FOUND);
        }
        let (status, body) =
            get_from("/api/cases/nope/solve", "203.0.113.250, 198.51.100.77").await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert!(body["error"].as_str().unwrap().contains("rate limit"));
    }

    #[test]
    fn validate_deltas_rejects_isolated_bus_delta() {
        // Patch an isolated (type 4) bus onto the case200 fixture: it exists in
        // the full network view but DcNetwork::from_network excludes it from the
        // model, so a delta there must 400 with an accurate message instead of
        // failing deep in the engine as a 500.
        let spec = &FALLBACK_SPECS[0];
        let start = spec.text.find("mpc.bus = [").unwrap();
        let end = start + spec.text[start..].find("\n];").unwrap();
        let mut patched = String::with_capacity(spec.text.len() + 64);
        patched.push_str(&spec.text[..end]);
        patched.push_str(
            "\n\t201\t 4\t 0.0\t 0.0\t 0.0\t 0.0\t 1\t 1.0\t 0.0\t 115.0\t 4\t 1.1\t 0.9;",
        );
        patched.push_str(&spec.text[end..]);

        let case = powerio::parse_str(&patched, "m").unwrap().network;
        let coords: Coords = case
            .buses
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id.0, (i as f64, 0.0)))
            .collect();
        let entry = build_entry("iso", "iso", case, coords, None, true).unwrap();

        let err = validate_deltas(&entry, &HashMap::from([(201usize, 1.0)])).unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert!(err.message.contains("isolated"), "got: {}", err.message);

        validate_deltas(&entry, &HashMap::from([(1usize, 1.0)])).unwrap();
    }

    #[tokio::test]
    async fn solve_stream_emits_solution_events() {
        let (status, headers, body) = get_raw("/api/cases/case200/solve?d=1:5").await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers[CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream"));
        let events: Vec<_> = body
            .lines()
            .filter_map(|line| line.strip_prefix("event:"))
            .map(str::trim)
            .collect();
        assert_eq!(events, ["status", "solution", "done"]);
        assert!(body.contains(r#""case":"case200""#));
        assert!(body.contains(r#""solve_ms":"#));
    }

    #[cfg(feature = "sensitivity")]
    #[tokio::test]
    async fn solve_stream_emits_sensitivity_when_requested() {
        let (status, headers, body) = get_raw("/api/cases/case200/solve?sens=1&d=1:5").await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers[CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream"));
        let events: Vec<_> = body
            .lines()
            .filter_map(|line| line.strip_prefix("event:"))
            .map(str::trim)
            .collect();
        assert_eq!(events, ["status", "solution", "sensitivity", "done"]);
        assert!(body.contains(r#""case":"case200""#));
        assert!(body.contains(r#""solve_ms":"#));
    }

    #[cfg(not(feature = "sensitivity"))]
    #[tokio::test]
    async fn solve_stream_rejects_sensitivity_without_feature() {
        let (status, body) = get("/api/cases/case200/solve?sens=1").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body["error"]
            .as_str()
            .unwrap()
            .contains("sensitivity disabled"));
    }

    #[tokio::test]
    async fn solve_stream_rejects_missing_bus() {
        let (status, body) = get("/api/cases/case200/solve?sens=999999").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("unknown bus"));
    }

    #[tokio::test]
    async fn fallback_requires_explicit_flag() {
        let err = match AppState::load(PathBuf::from("/definitely/no/tellegen/data"), false) {
            Ok(_) => panic!("fallback load should require TELLEGEN_ALLOW_FALLBACK=1"),
            Err(err) => err,
        };
        assert!(err.contains("TELLEGEN_ALLOW_FALLBACK=1"));
    }
}
