use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use metrics::{counter, gauge};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc, time::Instant};
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use treblle_axum::{Treblle, TreblleExt};

// API Types
#[derive(Debug, Serialize, Deserialize)]
struct ApiRequest {
    message: String,
    #[serde(default)]
    delay_ms: u64,
    #[serde(default)]
    sensitive_data: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiResponse {
    message: String,
    timestamp: String,
    processing_time_ms: u64,
}

#[derive(Debug, Serialize)]
struct DynamicResponse {
    id: String,
    message: String,
    timestamp: String,
}

// Metrics state
#[derive(Default)]
struct Metrics {
    requests_total: usize,
    processing_time_ms: Vec<u64>,
}

struct AppState {
    metrics: Arc<RwLock<Metrics>>,
    prometheus: PrometheusHandle,
}

// Regular route handlers
async fn handle_json(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ApiRequest>,
) -> Json<ApiResponse> {
    let start = Instant::now();

    if payload.delay_ms > 0 {
        tokio::time::sleep(tokio::time::Duration::from_millis(payload.delay_ms)).await;
    }

    let processing_time = start.elapsed().as_millis() as u64;
    {
        let mut metrics = state.metrics.write().await;
        metrics.requests_total += 1;
        metrics.processing_time_ms.push(processing_time);

        counter!("requests_total", "type" => "regular", "endpoint" => "json").increment(1);
        gauge!("request_processing_time", "type" => "regular", "endpoint" => "json")
            .set(processing_time as f64);
    }

    Json(ApiResponse {
        message: format!("Regular: {}", payload.message),
        timestamp: chrono::Utc::now().to_rfc3339(),
        processing_time_ms: processing_time,
    })
}

async fn handle_dynamic(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ApiRequest>,
) -> Json<DynamicResponse> {
    let start = Instant::now();

    if payload.delay_ms > 0 {
        tokio::time::sleep(tokio::time::Duration::from_millis(payload.delay_ms)).await;
    }

    let processing_time = start.elapsed().as_millis() as u64;
    {
        let mut metrics = state.metrics.write().await;
        metrics.requests_total += 1;
        metrics.processing_time_ms.push(processing_time);

        counter!("requests_total", "type" => "regular", "endpoint" => "dynamic").increment(1);
        gauge!("request_processing_time", "type" => "regular", "endpoint" => "dynamic")
            .set(processing_time as f64);
    }

    Json(DynamicResponse {
        id,
        message: payload.message,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

async fn handle_text(State(state): State<Arc<AppState>>, body: String) -> String {
    let start = Instant::now();
    let processing_time = start.elapsed().as_millis() as u64;

    {
        let mut metrics = state.metrics.write().await;
        metrics.requests_total += 1;
        metrics.processing_time_ms.push(processing_time);

        counter!("requests_total", "type" => "regular", "endpoint" => "text").increment(1);
        gauge!("request_processing_time", "type" => "regular", "endpoint" => "text")
            .set(processing_time as f64);
    }

    format!("Processed: {}", body)
}

// Health and metrics endpoints
async fn health_check() -> &'static str {
    "OK"
}

async fn metrics_handler(State(state): State<Arc<AppState>>) -> String {
    state.prometheus.render()
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt().with_env_filter("info,tower_http=debug").init();

    // Initialize metrics
    let prometheus_handle =
        PrometheusBuilder::new().install_recorder().expect("failed to install Prometheus recorder");

    // Initialize state
    let state = Arc::new(AppState {
        metrics: Arc::new(RwLock::new(Metrics::default())),
        prometheus: prometheus_handle,
    });

    // Create Treblle middleware
    let treblle = Treblle::new(
        std::env::var("TREBLLE_API_KEY").unwrap_or_else(|_| "test_key".to_string()),
        std::env::var("TREBLLE_PROJECT_ID").unwrap_or_else(|_| "test_project".to_string()),
    )
    .add_masked_fields(vec!["sensitive_data".to_string()])
    .add_ignored_routes(vec!["/health".to_string(), "/metrics".to_string()]);

    // Regular routes without Treblle
    let regular_routes = Router::new()
        .route("/api/json", post(handle_json))
        .route("/api/dynamic/:id", post(handle_dynamic))
        .route("/api/text", post(handle_text))
        .with_state(state.clone());

    // Routes with Treblle monitoring
    let monitored_routes = Router::new()
        .route("/api/with-treblle/json", post(handle_json))
        .route("/api/with-treblle/dynamic/:id", post(handle_dynamic))
        .route("/api/with-treblle/text", post(handle_text))
        .with_state(state.clone())
        .treblle(treblle);

    // Combined router
    let app = Router::new()
        .merge(regular_routes)
        .merge(monitored_routes)
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8082));
    println!("Axum test service listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
