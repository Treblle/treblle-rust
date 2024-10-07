//! Treblle integration for Rocket web framework.

mod fairing;

pub use fairing::TreblleFairing;

/// Creates a new Treblle fairing.
pub fn treblle_fairing(api_key: String, project_id: String) -> TreblleFairing {
    TreblleFairing::new(api_key, project_id)
}