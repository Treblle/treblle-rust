//! Certificate handling module for the Treblle middleware.
//!
//! This module provides functionality for loading root certificates,
//! either from a custom file specified in the configuration or from
//! the webpki-roots bundle.

use rustls::{OwnedTrustAnchor, RootCertStore};
use std::fs::File;
use std::io::BufReader;

use treblle_core::error::{Result, TreblleError};

use crate::logger::{log, LogLevel};

/// Loads root certificates into the provided `RootCertStore`.
///
/// # Arguments
///
/// * `root_store` - A mutable reference to the `RootCertStore` to load certificates into.
/// * `root_ca_path` - Optional path to a custom root CA certificate file.
///
/// # Returns
///
/// A `Result` indicating success or failure of the certificate loading process.
pub fn load_root_certs(
    root_store: &mut RootCertStore,
    root_ca_path: Option<&String>,
) -> Result<()> {
    if let Some(ca_path) = root_ca_path {
        log(LogLevel::Debug, &format!("Attempting to load custom root CA from: {ca_path}"));
        match load_custom_certificates(root_store, ca_path) {
            Ok(()) => {
                log(LogLevel::Debug, "Custom root CA loaded successfully");
                return Ok(());
            }
            Err(e) => {
                log(
                    LogLevel::Error,
                    &format!("Failed to load custom root CA: {e}. Falling back to webpki-roots."),
                );
            }
        }
    } else {
        log(LogLevel::Debug, "No custom root CA path provided, using webpki-roots");
    }

    load_webpki_roots(root_store);
    log(LogLevel::Debug, "Webpki root certificates loaded successfully");
    Ok(())
}

/// Loads custom certificates from a specified file path.
fn load_custom_certificates(root_store: &mut RootCertStore, ca_path: &str) -> Result<()> {
    log(LogLevel::Debug, &format!("Opening custom root CA file: {ca_path}"));
    let file = File::open(ca_path).map_err(|e| {
        let error_msg = format!("Failed to open custom root CA file: {e}");
        log(LogLevel::Error, &error_msg);
        TreblleError::Certificate(error_msg)
    })?;

    let mut reader = BufReader::new(file);
    log(LogLevel::Debug, "Parsing certificates from file");

    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| {
            let error_msg = format!("Failed to parse custom root CA file: {e}");
            log(LogLevel::Error, &error_msg);
            TreblleError::Certificate(error_msg)
        })?;

    if certs.is_empty() {
        let error_msg = "No certificates found in the custom root CA file";
        log(LogLevel::Error, error_msg);
        return Err(TreblleError::Certificate(error_msg.to_string()));
    }

    log(LogLevel::Debug, &format!("Found {} certificates", certs.len()));

    for (index, cert) in certs.iter().enumerate() {
        log(LogLevel::Debug, &format!("Adding certificate {} to store", index + 1));
        root_store.add(&rustls::Certificate(cert.to_vec())).map_err(|e| {
            let error_msg = format!("Failed to add custom root CA to store: {e}");
            log(LogLevel::Error, &error_msg);
            TreblleError::Certificate(error_msg)
        })?;
    }

    log(LogLevel::Debug, "Successfully added all certificates to store");
    Ok(())
}

/// Loads the default webpki-roots certificate bundle.
fn load_webpki_roots(root_store: &mut RootCertStore) {
    log(LogLevel::Debug, "Loading webpki root certificates");
    let trust_anchors = webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    });
    root_store.add_trust_anchors(trust_anchors);
    log(LogLevel::Debug, "Webpki root certificates loaded successfully");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_webpki_roots() {
        let mut root_store = RootCertStore::empty();
        load_webpki_roots(&mut root_store);
        assert!(!root_store.is_empty());
    }

    #[test]
    fn test_load_root_certs_without_path() {
        let mut root_store = RootCertStore::empty();
        assert!(load_root_certs(&mut root_store, None).is_ok());
        assert!(!root_store.is_empty());
    }

    #[test]
    fn test_load_root_certs_with_invalid_path() {
        let mut root_store = RootCertStore::empty();
        let invalid_path = String::from("/invalid/path/to/cert.pem");
        assert!(load_root_certs(&mut root_store, Some(&invalid_path)).is_ok()); // Should fallback to webpki
        assert!(!root_store.is_empty());
    }
}