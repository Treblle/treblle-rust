//! Treblle integration for Axum web framework.

mod middleware;

pub use middleware::TreblleLayer;

/// Creates a new Treblle middleware layer.
pub fn treblle_layer(api_key: String, project_id: String) -> TreblleLayer {
    TreblleLayer::new(api_key, project_id)
}