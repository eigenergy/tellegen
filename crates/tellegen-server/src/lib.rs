use std::{
    collections::{BTreeMap, HashMap, HashSet},
    convert::Infallible,
    env, fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
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
    CaseSpec::aux(
        "case2000",
        "ACTIVSg2000 (Texas)",
        "ACTIVSg2000/case_ACTIVSg2000.m",
        "ACTIVSg2000/ACTIVSg2000.aux",
    ),
    CaseSpec::cats_csv(
        "cats",
        "CATS (California)",
        "CATS/CaliforniaTestSystem.m",
        "CATS/CATS_buses.csv",
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
}

#[derive(Clone, Copy)]
enum CoordSpec {
    Aux(&'static str),
    CatsBusCsv(&'static str),
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
        }
    }

    const fn cats_csv(
        id: &'static str,
        name: &'static str,
        casefile: &'static str,
        csvfile: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            casefile,
            coords: CoordSpec::CatsBusCsv(csvfile),
        }
    }

    fn coord_file(self) -> &'static str {
        match self.coords {
            CoordSpec::Aux(path) | CoordSpec::CatsBusCsv(path) => path,
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
    pub path: [[f64; 2]; 2],
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
    pub bus: usize,
    pub units: &'static str,
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
        Self::load(data_dir(), allow_fallback())
    }

    pub fn load(data_dir: PathBuf, allow_fallback: bool) -> Result<Self, String> {
        let staged_specs: Vec<_> = CASE_SPECS
            .iter()
            .copied()
            .filter(|spec| staged(&data_dir, spec))
            .collect();
        let mut cases = BTreeMap::new();

        if staged_specs.len() == CASE_SPECS.len() {
            for spec in staged_specs {
                let entry = build_staged_entry(&data_dir, spec)?;
                cases.insert(entry.id.clone(), Arc::new(entry));
            }
        } else if allow_fallback {
            tracing::warn!(
                data_dir = %data_dir.display(),
                "staged case data incomplete; serving embedded pglib fallback cases"
            );
            for spec in FALLBACK_SPECS {
                let entry = build_fallback_entry(spec)?;
                cases.insert(entry.id.clone(), Arc::new(entry));
            }
        } else {
            let loaded: Vec<_> = staged_specs.iter().map(|s| s.id).collect();
            let missing: Vec<_> = CASE_SPECS
                .iter()
                .map(|s| s.id)
                .filter(|id| !loaded.contains(id))
                .collect();
            return Err(format!(
                "staged case data incomplete under {}; missing {}. Run scripts/stage-data.sh or set TELLEGEN_ALLOW_FALLBACK=1 for the pglib dev fallback.",
                data_dir.display(),
                missing.join(", ")
            ));
        }

        if cases.is_empty() {
            return Err(format!("no cases loaded from {}", data_dir.display()));
        }
        Ok(Self {
            cases: Arc::new(cases),
            solver_permits: Arc::new(Semaphore::new(solver_concurrency())),
            solver_timeout: solver_timeout(),
        })
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
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn router(state: Arc<AppState>, frontend_build: Option<PathBuf>) -> Router {
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/cases", get(cases))
        .route("/api/cases/{id}/case", get(case_network_json))
        .route("/api/cases/{id}/network", get(network))
        .route("/api/cases/{id}/solution", get(solution))
        .route("/api/cases/{id}/sensitivity/lmp/d/{bus}", get(sensitivity))
        .route("/api/cases/{id}/solve", get(solve_stream))
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

async fn sensitivity(
    State(state): State<Arc<AppState>>,
    AxumPath((id, bus)): AxumPath<(String, usize)>,
    Query(query): Query<DemandQuery>,
) -> ApiResult<Json<SensitivityPayload>> {
    let entry = state.case(&id)?;
    if !entry.network.buses.iter().any(|b| b.id.0 == bus) {
        return Err(ApiError::not_found(format!("unknown bus {bus}")));
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
        let request = build_request(&entry, deltas, Some(bus));
        let id_for_task = entry.id.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let output = run_solve_limited(state, entry, request, cancel).await?;
        let Some(m) = output.sensitivities.first() else {
            return Err(ApiError::not_found(format!("unknown bus {bus}")));
        };
        Ok(Json(sensitivity_payload(&id_for_task, m)))
    }
}

async fn solve_stream(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<DemandQuery>,
) -> ApiResult<Sse<ReceiverStream<Result<Event, Infallible>>>> {
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
    let request = build_request(&entry, deltas, query.sens);
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
/// deltas, plus (when a sens bus is given and maps to a dense column) a single
/// Price/Demand sensitivity cell — the dLMP/dd column the frontend reads.
fn build_request(
    entry: &CaseEntry,
    deltas: HashMap<usize, f64>,
    sens_bus: Option<usize>,
) -> SolveRequest {
    let mut request = SolveRequest::default();
    request.edits.deltas = deltas
        .into_iter()
        .map(|(bus, mw)| (bus as i64, mw))
        .collect();
    #[cfg(feature = "sensitivity")]
    if let Some(idx) = sens_bus.and_then(|bus| entry.dc.bus_ids.iter().position(|&id| id == bus)) {
        request.sensitivities = vec![SensRequest {
            operand: Operand::Price(Power::Active),
            parameter: Parameter::Demand(Power::Active),
            indices: Some(vec![idx]),
            mode: Mode::Auto,
        }];
    }
    #[cfg(not(feature = "sensitivity"))]
    let _ = (entry, sens_bus);
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
        CoordSpec::CatsBusCsv(csvfile) => load_cats_coords(&data_dir.join(csvfile), &case)?,
    };
    build_entry(spec.id, spec.name, case, coords, false)
}

fn load_cats_coords(path: &Path, case: &Network) -> Result<Coords, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| format!("{}: empty coordinate CSV", path.display()))?;
    let headers: Vec<_> = header.split(',').map(str::trim).collect();
    let col = |name: &str| {
        headers
            .iter()
            .position(|h| h.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("{}: missing {name} column", path.display()))
    };
    let bus_i = col("bus_i")?;
    let lat_i = col("Lat")?;
    let lon_i = col("Lon")?;
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
                "{}: missing coordinates for CATS bus {}",
                path.display(),
                bus.id.0
            ));
        }
    }
    Ok(coords)
}

fn build_fallback_entry(spec: &FallbackSpec) -> Result<CaseEntry, String> {
    let case = powerio::parse_str(spec.text, "m")
        .map_err(|e| format!("{} fallback parse failed: {e}", spec.id))?
        .network;
    let coords = synthetic_layout(&case, spec.bbox);
    build_entry(spec.id, spec.name, case, coords, true)
}

fn build_entry(
    id: &str,
    name: &str,
    network: Network,
    coords: Coords,
    synthetic_coords: bool,
) -> Result<CaseEntry, String> {
    let network_json = network.to_json().map_err(|e| e.to_string())?;
    let dc = Arc::new(DcNetwork::from_network(&network)?);
    let base = solve_prebuilt(&dc, &SolveRequest::default())?;
    let view = network_payload(id, name, &network, &coords, synthetic_coords)?;
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
                path: [[from_lon, from_lat], [to_lon, to_lat]],
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

/// The dLMP/dd column from the requested Price/Demand cell: rows are buses, the single
/// column is the sens bus. Bus ids come from the matrix's self-describing metadata.
#[cfg(feature = "sensitivity")]
fn sensitivity_payload(case: &str, m: &SensitivityMatrix) -> SensitivityPayload {
    let bus = m.cols.first().map_or(0, |c| match c.element {
        ElementId::Bus(b) => b,
        _ => 0,
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
        parameter: "d",
        bus,
        units: "($/MWh)/MW",
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

fn allow_fallback() -> bool {
    matches!(
        env::var("TELLEGEN_ALLOW_FALLBACK")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
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
    use std::sync::OnceLock;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::ServiceExt;

    fn fallback_state() -> Arc<AppState> {
        static STATE: OnceLock<Arc<AppState>> = OnceLock::new();
        Arc::clone(STATE.get_or_init(|| {
            Arc::new(AppState::load(PathBuf::from("/definitely/no/tellegen/data"), true).unwrap())
        }))
    }

    async fn get_raw(path: &str) -> (StatusCode, HeaderMap, String) {
        let res = router(fallback_state(), None)
            .oneshot(
                axum::http::Request::builder()
                    .uri(path)
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
        assert!(body["values"].as_array().unwrap().len() >= 200);
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
