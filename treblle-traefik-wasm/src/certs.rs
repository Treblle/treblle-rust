use rustls::{OwnedTrustAnchor, RootCertStore};
use std::fs::File;
use std::io::BufReader;

use treblle_core::error::{Result, TreblleError};

use crate::host_functions::host_log;
use crate::constants::log_level;
use crate::CONFIG;

pub fn load_root_certs(root_store: &mut RootCertStore) -> Result<()> {
    if let Some(ca_path) = &CONFIG.root_ca_path {
        match load_custom_certificates(root_store, ca_path) {
            Ok(()) => {
                host_log(log_level::DEBUG, "Custom root CA loaded successfully");
                return Ok(());
            }
            Err(e) => {
                host_log(
                    log_level::ERROR,
                    &format!("Failed to load custom root CA: {e}. Falling back to webpki-roots."),
                );
            }
        }
    }

    load_webpki_roots(root_store);
    host_log(log_level::DEBUG, "Webpki root certificates loaded successfully");
    Ok(())
}

fn load_custom_certificates(root_store: &mut RootCertStore, ca_path: &str) -> Result<()> {
    let file = File::open(ca_path).map_err(|e| {
        host_log(log_level::ERROR, &format!("Failed to open custom root CA file: {e}"));
        TreblleError::Certificate(format!("Failed to open custom root CA file: {e}"))
    })?;

    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| {
            host_log(log_level::ERROR, &format!("Failed to parse custom root CA file: {e}"));
            TreblleError::Certificate(format!("Failed to parse custom root CA file: {e}"))
        })?;

    if certs.is_empty() {
        host_log(log_level::ERROR, "No certificates found in the custom root CA file");
        return Err(TreblleError::Certificate(
            "No certificates found in the custom root CA file".to_string(),
        ));
    }

    for cert in certs {
        root_store.add(&rustls::Certificate(cert.to_vec())).map_err(|e| {
            host_log(log_level::ERROR, &format!("Failed to add custom root CA to store: {e}"));
            TreblleError::Certificate(format!("Failed to add custom root CA to store: {e}"))
        })?;
    }

    Ok(())
}

fn load_webpki_roots(root_store: &mut RootCertStore) {
    root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
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
}