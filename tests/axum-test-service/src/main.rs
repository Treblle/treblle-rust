use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tokio_metrics::{TaskMetrics, TaskMonitor};
use tower_http::trace::TraceLayer;
use tracing::info;
use treblle_axum::{AxumConfig, Treblle, TreblleExt};

#[derive(Debug, Serialize, Clone, Deserialize)]
struct SensitiveData {
    password: String,
    credit_card: String,
    ssn: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiRequest {
    message: String,
    #[serde(default)]
    delay_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    sensitive_data: Option<SensitiveData>,
}

#[derive(Debug, Serialize, Clone)]
struct ApiResponse {
    message: String,
    timestamp: String,
    processing_time_ms: u64,
    request_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sensitive_data: Option<SensitiveData>,
}

#[derive(Debug, Serialize, Clone)]
struct ErrorResponse {
    error: String,
    code: String,
    timestamp: String,
}

#[derive(Clone)]
struct AppState {
    metrics: Arc<RwLock<ServiceMetrics>>,
    prometheus: PrometheusHandle,
}

// Also need to update the ServiceMetrics struct to match:
#[derive(Debug, Default, Serialize, Clone)]
struct ServiceMetrics {
    // Request counts
    regular_requests: u64,
    monitored_requests: u64,
    json_requests: u64,
    non_json_requests: u64,

    // Processing times (in milliseconds)
    regular_processing_times: Vec<f64>,
    monitored_processing_times: Vec<f64>,
    middleware_overhead_times: Vec<f64>,

    // Success/Error tracking
    success_count: u64,
    failure_count: u64,
    errors: u64,

    // Payload sizes (in bytes)
    request_sizes: Vec<usize>,
    response_sizes: Vec<usize>,
}

impl ServiceMetrics {
    fn avg_processing_time(&self, request_type: &str) -> Option<f64> {
        match request_type {
            "regular" => {
                if self.regular_processing_times.is_empty() {
                    None
                } else {
                    Some(
                        self.regular_processing_times.iter().sum::<f64>()
                            / self.regular_processing_times.len() as f64,
                    )
                }
            }
            "monitored" => {
                if self.monitored_processing_times.is_empty() {
                    None
                } else {
                    Some(
                        self.monitored_processing_times.iter().sum::<f64>()
                            / self.monitored_processing_times.len() as f64,
                    )
                }
            }
            _ => None,
        }
    }

    fn avg_middleware_overhead(&self) -> Option<f64> {
        if self.middleware_overhead_times.is_empty() {
            None
        } else {
            Some(
                self.middleware_overhead_times.iter().sum::<f64>()
                    / self.middleware_overhead_times.len() as f64,
            )
        }
    }

    fn success_rate(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            0.0
        } else {
            (self.success_count as f64 / total as f64) * 100.0
        }
    }

    fn avg_request_size(&self) -> Option<f64> {
        if self.request_sizes.is_empty() {
            None
        } else {
            Some(self.request_sizes.iter().sum::<usize>() as f64 / self.request_sizes.len() as f64)
        }
    }

    fn avg_response_size(&self) -> Option<f64> {
        if self.response_sizes.is_empty() {
            None
        } else {
            Some(
                self.response_sizes.iter().sum::<usize>() as f64 / self.response_sizes.len() as f64,
            )
        }
    }
}

async fn init_tokio_metrics() -> TaskMonitor {
    let monitor = TaskMonitor::new();
    let metrics_hdl = monitor.clone();

    tokio::spawn(async move {
        for metrics in metrics_hdl.intervals() {
            let TaskMetrics {
                instrumented_count,
                dropped_count,
                total_fast_poll_count,
                total_slow_poll_count,
                total_poll_duration,
                total_scheduled_count,
                total_scheduled_duration,
                ..
            } = metrics;

            // Task metrics
            gauge!("tokio_tasks_instrumented_total").set(instrumented_count as f64);
            gauge!("tokio_tasks_dropped_total").set(dropped_count as f64);

            // Poll metrics
            gauge!("tokio_poll_count_fast_total").set(total_fast_poll_count as f64);
            gauge!("tokio_poll_count_slow_total").set(total_slow_poll_count as f64);
            gauge!("tokio_poll_duration_ms").set(total_poll_duration.as_millis() as f64);

            // Scheduling metrics
            gauge!("tokio_scheduled_count_total").set(total_scheduled_count as f64);
            gauge!("tokio_scheduled_duration_ms").set(total_scheduled_duration.as_millis() as f64);

            // Derived metrics
            if total_scheduled_count > 0 {
                gauge!("tokio_task_completion_rate")
                    .set(dropped_count as f64 / total_scheduled_count as f64);
            }

            if total_fast_poll_count + total_slow_poll_count > 0 {
                gauge!("tokio_poll_efficiency").set(
                    total_fast_poll_count as f64
                        / (total_fast_poll_count + total_slow_poll_count) as f64,
                );
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    monitor
}

fn init_metrics() {
    // Request metrics
    describe_counter!("requests_total", "Total number of requests processed");
    describe_counter!("requests_success_total", "Total number of successful requests");
    describe_counter!("requests_error_total", "Total number of failed requests");

    // Timing metrics
    describe_histogram!("request_duration_ms", "Request duration in milliseconds");
    describe_histogram!("request_processing_ms", "Request processing time");
    describe_histogram!("middleware_overhead_ms", "Additional time added by middleware");

    // Size metrics
    describe_histogram!("request_body_size_bytes", "Request payload size in bytes");
    describe_histogram!("response_body_size_bytes", "Response payload size in bytes");

    // Tokio runtime metrics
    describe_gauge!("tokio_tasks_instrumented_total", "Total number of instrumented tasks");
    describe_gauge!("tokio_tasks_dropped_total", "Total number of dropped tasks");
    describe_gauge!("tokio_poll_count_fast_total", "Total number of fast polls");
    describe_gauge!("tokio_poll_count_slow_total", "Total number of slow polls");
    describe_gauge!("tokio_poll_duration_ms", "Total poll duration in milliseconds");
    describe_gauge!("tokio_scheduled_count_total", "Total number of scheduled tasks");
    describe_gauge!("tokio_scheduled_duration_ms", "Total scheduled duration in milliseconds");
    describe_gauge!("tokio_task_completion_rate", "Rate of task completion");
    describe_gauge!("tokio_poll_efficiency", "Ratio of fast polls to total polls");
}

async fn process_request(
    request_type: &str,
    State(state): State<AppState>,
    payload: Json<ApiRequest>,
) -> Json<ApiResponse> {
    let start = Instant::now();
    let request_type = request_type.to_string();
    let request_size = serde_json::to_vec(&payload.0).map(|v| v.len()).unwrap_or(0);

    // Record request size
    histogram!("request_size_bytes", "type" => request_type.clone()).record(request_size as f64);

    // Record request start
    counter!("requests_total", "type" => request_type.clone()).increment(1);

    // Track request in progress
    gauge!("requests_in_progress", "type" => request_type.clone()).increment(1.0);

    // Simulate processing delay if specified
    if payload.delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(payload.delay_ms)).await;
    }

    let duration = start.elapsed();
    let duration_ms = duration.as_secs_f64() * 1000.0;

    // Record detailed timing metrics
    histogram!("request_duration_ms", "type" => request_type.clone()).record(duration_ms);
    histogram!("processing_time_ms", "type" => request_type.clone()).record(duration_ms);

    let response = ApiResponse {
        message: payload.message.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        processing_time_ms: duration.as_millis() as u64,
        request_type: request_type.clone(),
        sensitive_data: payload.sensitive_data.clone(),
    };

    // Record response size
    let response_size = serde_json::to_vec(&response).map(|v| v.len()).unwrap_or(0);
    histogram!("response_size_bytes", "type" => request_type.clone()).record(response_size as f64);

    // Update success rate
    counter!("requests_success_total", "type" => request_type.clone()).increment(1);

    // Decrease in-progress counter
    gauge!("requests_in_progress", "type" => request_type.clone()).decrement(1.0);

    // Update state metrics
    let mut metrics = state.metrics.write().await;
    match request_type.as_str() {
        "regular" => {
            metrics.regular_requests += 1;
            metrics.regular_processing_times.push(duration_ms);
            metrics.request_sizes.push(request_size);
            metrics.response_sizes.push(response_size);
            metrics.success_count += 1;
        }
        "monitored" => {
            metrics.monitored_requests += 1;
            metrics.monitored_processing_times.push(duration_ms);
            // Calculate middleware overhead
            if let Some(regular_avg) = metrics.regular_processing_times.last() {
                let overhead = duration_ms - regular_avg;
                metrics.middleware_overhead_times.push(overhead);
                histogram!("middleware_overhead_ms").record(overhead);
            }
            metrics.success_count += 1;
        }
        _ => {}
    }

    Json(response)
}

async fn handle_regular_json(
    State(state): State<AppState>,
    payload: Json<ApiRequest>,
) -> Json<ApiResponse> {
    process_request("regular", State(state), payload).await
}

async fn handle_monitored_json(
    State(state): State<AppState>,
    payload: Json<ApiRequest>,
) -> Json<ApiResponse> {
    process_request("monitored", State(state), payload).await
}

async fn handle_error() -> Json<ErrorResponse> {
    counter!("errors_total").increment(1);
    counter!("requests_error_total").increment(1);

    let error_response = ErrorResponse {
        error: "Simulated error".to_string(),
        code: "ERR_SIMULATED".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    Json(error_response)
}

async fn health_check() -> &'static str {
    counter!("health_checks_total").increment(1);
    info!("Health check called");
    "OK"
}

async fn get_stats(State(state): State<AppState>) -> Json<ServiceMetrics> {
    let metrics = state.metrics.read().await;
    Json(ServiceMetrics {
        // Request counts
        regular_requests: metrics.regular_requests,
        monitored_requests: metrics.monitored_requests,
        json_requests: metrics.json_requests,
        non_json_requests: metrics.non_json_requests,

        // Processing times
        regular_processing_times: metrics.regular_processing_times.clone(),
        monitored_processing_times: metrics.monitored_processing_times.clone(),
        middleware_overhead_times: metrics.middleware_overhead_times.clone(),

        // Success/Error tracking
        success_count: metrics.success_count,
        failure_count: metrics.failure_count,
        errors: metrics.errors,

        // Payload sizes
        request_sizes: metrics.request_sizes.clone(),
        response_sizes: metrics.response_sizes.clone(),
    })
}

async fn metrics_handler(State(state): State<AppState>) -> String {
    state.prometheus.render()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .init();

    init_tokio_metrics().await;
    init_metrics();

    let prometheus_handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("processing_time_ms".to_string()),
            &[1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0],
        )
        .unwrap()
        .set_buckets_for_metric(
            Matcher::Full("middleware_overhead_ms".to_string()),
            &[0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 25.0, 50.0],
        )
        .unwrap()
        .set_buckets_for_metric(
            Matcher::Full("request_duration_ms".to_string()),
            &[1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0],
        )
        .unwrap()
        .install_recorder()
        .unwrap();

    let state = AppState {
        metrics: Arc::new(RwLock::new(ServiceMetrics::default())),
        prometheus: prometheus_handle,
    };

    let config = AxumConfig::builder()
        .api_key(std::env::var("TREBLLE_API_KEY").unwrap_or_else(|_| "test_key".to_string()))
        .project_id(
            std::env::var("TREBLLE_PROJECT_ID").unwrap_or_else(|_| "test_project".to_string()),
        )
        .set_api_urls(vec![std::env::var("TREBLLE_API_URL")
            .unwrap_or_else(|_| "http://mock-treblle-api:4321".to_string())])
        .add_masked_fields(vec![
            "password".to_string(),
            "credit_card".to_string(),
            "ssn".to_string(),
        ])
        .add_ignored_routes(vec![
            "/health".to_string(),
            "/metrics".to_string(),
            "/stats".to_string(),
        ])
        .build()
        .unwrap();

    let treblle = Treblle::from_config(config);

    let regular_routes = Router::new()
        .route("/api/json", post(handle_regular_json))
        .route("/api/error", get(handle_error));

    let monitored_routes = Router::new()
        .route("/api/with-treblle/json", post(handle_monitored_json))
        .route("/api/with-treblle/error", get(handle_error))
        .treblle(treblle);

    let app = Router::new()
        .merge(regular_routes)
        .merge(monitored_routes)
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .route("/stats", get(get_stats))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8082));
    info!("Axum test service listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.expect("Failed to bind to address");

    axum::serve(listener, app).await.expect("Failed to start server");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    // Create a mock recorder for testing
    fn setup_test_metrics() -> PrometheusHandle {
        static METRICS_INIT: std::sync::Once = std::sync::Once::new();
        static mut HANDLE: Option<PrometheusHandle> = None;

        unsafe {
            METRICS_INIT.call_once(|| {
                let handle = PrometheusBuilder::new()
                    .install_recorder()
                    .expect("failed to install test metrics recorder");
                HANDLE = Some(handle);
            });
            HANDLE.clone().unwrap()
        }
    }

    #[tokio::test]
    async fn test_health_check() {
        let state = AppState {
            metrics: Arc::new(RwLock::new(ServiceMetrics::default())),
            prometheus: setup_test_metrics(),
        };

        let app = Router::new().route("/health", get(health_check)).with_state(state);

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_json_endpoints() {
        let state = AppState {
            metrics: Arc::new(RwLock::new(ServiceMetrics::default())),
            prometheus: setup_test_metrics(),
        };

        let config =
            AxumConfig::builder().api_key("test_key").project_id("test_project").build().unwrap();
        let treblle = Treblle::from_config(config);

        let app = Router::new()
            .route("/api/json", post(handle_regular_json))
            .route("/api/with-treblle/json", post(handle_monitored_json))
            .treblle(treblle)
            .with_state(state.clone());

        let payload = ApiRequest { message: "test".to_string(), delay_ms: 0, sensitive_data: None };

        // Test regular endpoint
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/json")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Test monitored endpoint
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/with-treblle/json")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
