use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use serde_json::{json, Value};
use tokio::{sync::RwLock, time::Instant};
use tower_http::trace::TraceLayer;
use tracing::{debug, info, warn};
use treblle_core::schema::TrebllePayload;

#[derive(Clone)]
struct AppState {
    metrics: Arc<RwLock<TreblleApiMetrics>>,
    prometheus: PrometheusHandle,
}

#[derive(Debug, Default)]
struct PayloadValidationMetrics {
    total_requests: usize,
    valid_requests: usize,
    invalid_requests: usize,
    masked_fields_count: usize,
    processing_times: Vec<Duration>,
    payload_sizes: Vec<usize>,
    schema_validation_errors: Vec<String>,
}

#[derive(Debug, Default)]
struct TreblleApiMetrics {
    validation: PayloadValidationMetrics,
    endpoints: EndpointMetrics,
}

#[derive(Debug, Default)]
struct EndpointMetrics {
    rocknrolla_hits: usize,
    punisher_hits: usize,
    sicario_hits: usize,
}

fn count_masked_fields(value: &Value) -> usize {
    match value {
        Value::Object(map) => map.iter().fold(0, |count, (key, val)| {
            let mut total = count;
            if is_sensitive_field(key) && val.as_str() == Some("*****") {
                total += 1;
            }
            total + count_masked_fields(val)
        }),
        Value::Array(arr) => arr.iter().map(count_masked_fields).sum(),
        _ => 0,
    }
}

fn is_sensitive_field(field: &str) -> bool {
    let sensitive_patterns = [
        "password",
        "pwd",
        "secret",
        "password_confirmation",
        "cc",
        "card",
        "credit",
        "ccv",
        "cvv",
        "ssn",
        "social",
    ];
    let field_lower = field.to_lowercase();
    sensitive_patterns.iter().any(|&pattern| field_lower.contains(pattern))
}

fn validate_schema(payload: &TrebllePayload) -> Result<(), Box<dyn std::error::Error>> {
    // Basic validation checks
    if payload.api_key.is_empty() {
        return Err("API key is required".into());
    }
    if payload.project_id.is_empty() {
        return Err("Project ID is required".into());
    }
    if payload.version <= 0.0 {
        return Err("Invalid version number".into());
    }

    // Validate request data
    if let Some(body) = &payload.data.request.body {
        if !body.is_object() && !body.is_array() {
            return Err("Request body must be a JSON object or array".into());
        }
    }

    // Validate server info
    if payload.data.server.ip.is_empty() {
        return Err("Server IP is required".into());
    }
    if payload.data.server.protocol.is_empty() {
        return Err("Server protocol is required".into());
    }

    // Validate request info
    let request = &payload.data.request;
    if request.url.is_empty() {
        return Err("Request URL is required".into());
    }
    if request.method.is_empty() {
        return Err("Request method is required".into());
    }

    // Validate response info
    if payload.data.response.code == 0 {
        return Err("Response code is required".into());
    }

    // Validate language info
    if payload.data.language.name.is_empty() {
        return Err("Language name is required".into());
    }

    Ok(())
}

async fn handle_treblle_payload(
    State(state): State<AppState>,
    endpoint: &str,
    Json(payload): Json<TrebllePayload>,
) -> Json<Value> {
    let start = Instant::now();
    let mut metrics = state.metrics.write().await;
    metrics.validation.total_requests += 1;

    // Track endpoint distribution
    match endpoint {
        "rocknrolla" => metrics.endpoints.rocknrolla_hits += 1,
        "punisher" => metrics.endpoints.punisher_hits += 1,
        "sicario" => metrics.endpoints.sicario_hits += 1,
        _ => warn!("Unknown endpoint accessed: {}", endpoint),
    }

    // Validate payload
    let payload_size = serde_json::to_string(&payload).map(|s| s.len()).unwrap_or_default();
    metrics.validation.payload_sizes.push(payload_size);

    // Check for masked fields in request data
    if let Some(body) = &payload.data.request.body {
        let masked_count = count_masked_fields(body);
        metrics.validation.masked_fields_count += masked_count;
        gauge!("treblle_masked_fields_total").set(masked_count as f64);
    }

    // Validate schema requirements
    let validation_result = validate_schema(&payload);
    if validation_result.is_ok() {
        metrics.validation.valid_requests += 1;
        counter!("treblle_valid_requests_total").increment(1);
    } else {
        metrics.validation.invalid_requests += 1;
        counter!("treblle_invalid_requests_total").increment(1);
        if let Err(err) = validation_result {
            metrics.validation.schema_validation_errors.push(err.to_string());
        }
    }

    let processing_time = start.elapsed();
    metrics.validation.processing_times.push(processing_time);
    histogram!("treblle_processing_time_ms").record(processing_time.as_millis() as f64);

    gauge!("treblle_payload_size_bytes").set(payload_size as f64);

    debug!(
        "Processed Treblle payload: endpoint={}, size={}, time={:?}",
        endpoint, payload_size, processing_time
    );

    Json(json!({
        "status": "success",
        "message": "Payload processed successfully",
        "metrics": {
            "size_bytes": payload_size,
            "processing_time_ms": processing_time.as_millis(),
            "masked_fields": metrics.validation.masked_fields_count
        }
    }))
}

async fn handle_rocknrolla(state: State<AppState>, payload: Json<TrebllePayload>) -> Json<Value> {
    handle_treblle_payload(state, "rocknrolla", payload).await
}

async fn handle_punisher(state: State<AppState>, payload: Json<TrebllePayload>) -> Json<Value> {
    handle_treblle_payload(state, "punisher", payload).await
}

async fn handle_sicario(state: State<AppState>, payload: Json<TrebllePayload>) -> Json<Value> {
    handle_treblle_payload(state, "sicario", payload).await
}

async fn health_check() -> &'static str {
    info!("Health check called");
    "OK"
}

async fn metrics_handler(State(state): State<AppState>) -> String {
    state.prometheus.render()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .init();

    // Initialize metrics
    let prometheus_handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("treblle_processing_time_ms".to_string()),
            &[1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0],
        )?
        .install_recorder()?;

    let state = AppState {
        metrics: Arc::new(RwLock::new(TreblleApiMetrics::default())),
        prometheus: prometheus_handle,
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .route("/rocknrolla", post(handle_rocknrolla))
        .route("/punisher", post(handle_punisher))
        .route("/sicario", post(handle_sicario))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 4321));
    info!("Mock Treblle API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::{get, post},
        Router,
    };
    use metrics_exporter_prometheus::PrometheusBuilder;
    use tokio::sync::RwLock;
    use tower::ServiceExt;
    use treblle_core::schema::TrebllePayload;

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

    async fn setup_test_state() -> AppState {
        AppState {
            metrics: Arc::new(RwLock::new(TreblleApiMetrics::default())),
            prometheus: setup_test_metrics(),
        }
    }

    #[tokio::test]
    async fn test_health_check() {
        let state = setup_test_state().await;
        let app = Router::new().route("/health", get(health_check)).with_state(state);

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_valid_payload_processing() {
        let state = setup_test_state().await;
        let app =
            Router::new().route("/rocknrolla", post(handle_rocknrolla)).with_state(state.clone());

        let payload = TrebllePayload {
            api_key: "test_key".to_string(),
            project_id: "test_project".to_string(),
            version: 1.0,
            sdk: "rust-test".to_string(),
            data: treblle_core::schema::PayloadData {
                server: treblle_core::schema::ServerInfo {
                    ip: "127.0.0.1".to_string(),
                    timezone: "UTC".to_string(),
                    software: None,
                    signature: None,
                    protocol: "HTTP/1.1".to_string(),
                    encoding: None,
                    os: treblle_core::schema::OsInfo::default(),
                },
                language: treblle_core::schema::LanguageInfo {
                    name: "Rust".to_string(),
                    version: "1.0".to_string(),
                },
                request: treblle_core::schema::RequestInfo {
                    timestamp: chrono::Utc::now(),
                    ip: "127.0.0.1".to_string(),
                    url: "http://test.com".to_string(),
                    user_agent: "test-agent".to_string(),
                    method: "POST".to_string(),
                    headers: Default::default(),
                    body: Some(serde_json::json!({"test": "value"})),
                },
                response: treblle_core::schema::ResponseInfo {
                    headers: Default::default(),
                    code: 200,
                    size: 100,
                    load_time: 0.1,
                    body: Some(serde_json::json!({"status": "ok"})),
                },
                errors: vec![],
            },
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rocknrolla")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let metrics = state.metrics.read().await;
        assert_eq!(metrics.validation.total_requests, 1, "Total requests should be 1");
        assert_eq!(metrics.validation.valid_requests, 1, "Valid requests should be 1");
        assert_eq!(metrics.endpoints.rocknrolla_hits, 1, "Rocknrolla hits should be 1");
        assert!(
            !metrics.validation.processing_times.is_empty(),
            "Should have recorded processing time"
        );
        assert!(!metrics.validation.payload_sizes.is_empty(), "Should have recorded payload size");
    }
}

#[cfg(test)]
mod util_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_sensitive_field() {
        let sensitive_fields = vec![
            "password",
            "user_password",
            "PASSWORD",
            "credit_card",
            "ssn_number",
            "secret_key",
            "cc_number",
            "cvv_code",
        ];

        let non_sensitive_fields = vec!["username", "email", "address", "phone", "public_key"];

        for field in sensitive_fields {
            assert!(is_sensitive_field(field), "Field '{}' should be considered sensitive", field);
        }

        for field in non_sensitive_fields {
            assert!(
                !is_sensitive_field(field),
                "Field '{}' should not be considered sensitive",
                field
            );
        }
    }

    #[test]
    fn test_count_masked_fields() {
        let test_cases = vec![
            (
                json!({
                    "username": "test",
                    "password": "*****",
                    "nested": {
                        "credit_card": "*****",
                        "public": "visible"
                    }
                }),
                2,
                "Basic nested object",
            ),
            (
                json!({
                    "data": [{
                        "ssn": "*****"
                    }]
                }),
                1,
                "Array nested object",
            ),
            (
                json!({
                    "regular": "field",
                    "another": "value"
                }),
                0,
                "No sensitive fields",
            ),
        ];

        for (input, expected, test_name) in test_cases {
            assert_eq!(count_masked_fields(&input), expected, "Failed test case: {}", test_name);
        }
    }

    #[test]
    fn test_validate_schema() {
        fn create_test_payload(
            api_key: String,
            project_id: String,
            version: f32,
        ) -> TrebllePayload {
            TrebllePayload {
                api_key,
                project_id,
                version,
                sdk: "rust-test".to_string(),
                data: treblle_core::schema::PayloadData {
                    server: treblle_core::schema::ServerInfo {
                        ip: "127.0.0.1".to_string(),
                        timezone: "UTC".to_string(),
                        software: None,
                        signature: None,
                        protocol: "HTTP/1.1".to_string(),
                        encoding: None,
                        os: treblle_core::schema::OsInfo::default(),
                    },
                    language: treblle_core::schema::LanguageInfo {
                        name: "Rust".to_string(),
                        version: "1.0".to_string(),
                    },
                    request: treblle_core::schema::RequestInfo {
                        timestamp: chrono::Utc::now(),
                        ip: "127.0.0.1".to_string(),
                        url: "http://test.com".to_string(),
                        user_agent: "test-agent".to_string(),
                        method: "POST".to_string(),
                        headers: Default::default(),
                        body: Some(json!({"test": "value"})),
                    },
                    response: treblle_core::schema::ResponseInfo {
                        headers: Default::default(),
                        code: 200,
                        size: 100,
                        load_time: 0.1,
                        body: Some(json!({"status": "ok"})),
                    },
                    errors: vec![],
                },
            }
        }

        // Test valid payload
        let valid_payload =
            create_test_payload("test_key".to_string(), "test_project".to_string(), 1.0);
        assert!(validate_schema(&valid_payload).is_ok());

        // Test invalid payloads
        let invalid_payload = create_test_payload("".to_string(), "test_project".to_string(), 1.0);
        assert!(validate_schema(&invalid_payload).is_err());

        let invalid_payload =
            create_test_payload("test_key".to_string(), "test_project".to_string(), 0.0);
        assert!(validate_schema(&invalid_payload).is_err());

        // Test invalid request URL
        let mut invalid_payload =
            create_test_payload("test_key".to_string(), "test_project".to_string(), 1.0);
        invalid_payload.data.request.url = "".to_string();
        assert!(validate_schema(&invalid_payload).is_err());
    }
}
