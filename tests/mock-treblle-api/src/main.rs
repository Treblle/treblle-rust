use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{net::SocketAddr, sync::Arc, time::Instant};
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing::{debug, info, warn};

// API Metrics tracking
#[derive(Default)]
struct ApiMetrics {
    total_requests: usize,
    valid_requests: usize,
    invalid_requests: usize,
    masked_fields_detected: usize,
    payload_sizes: Vec<usize>,
    processing_times_ms: Vec<u64>,
}

// Treblle payload structure
#[derive(Debug, Deserialize)]
struct TrebllePayload {
    api_key: String,
    project_id: String,
    version: f32,
    sdk: String,
    data: TreblleData,
}

#[derive(Debug, Deserialize)]
struct TreblleData {
    server: Value,
    language: Value,
    request: Value,
    response: Value,
    #[serde(default)]
    errors: Vec<Value>,
}

#[derive(Debug, Serialize)]
struct ApiResponse {
    status: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Vec<String>>,
}

struct AppState {
    metrics: Arc<RwLock<ApiMetrics>>,
    prometheus: PrometheusHandle,
}

// Validation functions
fn validate_payload(payload: &TrebllePayload) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if payload.api_key.is_empty() {
        errors.push("API key is required".to_string());
    }
    if payload.project_id.is_empty() {
        errors.push("Project ID is required".to_string());
    }
    if payload.version <= 0.0 {
        errors.push("Invalid version number".to_string());
    }
    if payload.sdk.is_empty() {
        errors.push("SDK information is required".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn count_masked_fields(data: &Value) -> usize {
    let json_str = serde_json::to_string(data).unwrap_or_default();
    json_str.matches("*****").count()
}

// Request handlers
async fn handle_treblle_payload(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TrebllePayload>,
) -> Json<ApiResponse> {
    let start = Instant::now();
    let payload_size = serde_json::to_string(&payload).unwrap_or_default().len();

    // Update basic metrics
    counter!("treblle_requests_total").increment(1);
    gauge!("treblle_payload_size_bytes").set(payload_size as f64);

    // Validate payload
    match validate_payload(&payload) {
        Ok(()) => {
            let masked_fields = count_masked_fields(&payload.data.request);
            let processing_time = start.elapsed().as_millis() as u64;

            // Update detailed metrics
            {
                let mut metrics = state.metrics.write().await;
                metrics.total_requests += 1;
                metrics.valid_requests += 1;
                metrics.masked_fields_detected += masked_fields;
                metrics.payload_sizes.push(payload_size);
                metrics.processing_times_ms.push(processing_time);
            }

            counter!("treblle_valid_requests").increment(1);
            gauge!("treblle_masked_fields").set(masked_fields as f64);
            histogram!("treblle_processing_time_ms").record(processing_time as f64);

            debug!(
                "Processed valid Treblle payload: size={}b, masked_fields={}, time={}ms",
                payload_size, masked_fields, processing_time
            );

            Json(ApiResponse {
                status: "success".to_string(),
                message: "Payload processed successfully".to_string(),
                errors: None,
            })
        }
        Err(errors) => {
            // Update error metrics
            {
                let mut metrics = state.metrics.write().await;
                metrics.total_requests += 1;
                metrics.invalid_requests += 1;
            }

            counter!("treblle_invalid_requests").increment(1);

            warn!("Invalid Treblle payload: {:?}", errors);

            Json(ApiResponse {
                status: "error".to_string(),
                message: "Validation failed".to_string(),
                errors: Some(errors),
            })
        }
    }
}

async fn health_check() -> &'static str {
    "OK"
}

async fn metrics_handler(State(state): State<Arc<AppState>>) -> String {
    state.prometheus.render()
}

async fn get_metrics(State(state): State<Arc<AppState>>) -> Json<ApiMetrics> {
    Json((*state.metrics.read().await).clone())
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt().with_env_filter("debug,tower_http=debug").init();

    // Initialize metrics
    let builder = PrometheusBuilder::new();
    let builder = builder
        .set_buckets(&[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0])
        .unwrap();

    let handle = builder.install_recorder().expect("failed to install Prometheus recorder");

    // Initialize state
    let state = Arc::new(AppState {
        metrics: Arc::new(RwLock::new(ApiMetrics::default())),
        prometheus: handle,
    });

    // Build router
    let app = Router::new()
        .route("/", post(handle_treblle_payload))
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .route("/api/metrics", get(get_metrics))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 4321));
    info!("Mock Treblle API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
