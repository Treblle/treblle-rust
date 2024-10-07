//! Utility functions for Treblle integrations.

use regex::Regex;
use serde_json::{Map, Value};
use crate::error::{Result, TreblleError};

use http::header;

/// Check if a given content type is JSON.
pub fn is_json(content_type: &str) -> bool {
    content_type.to_lowercase().contains("application/json")
}

/// Mask sensitive data in a JSON value based on a regex pattern.
pub fn mask_sensitive_data(data: &Value, sensitive_keys_regex: &str) -> Result<Value> {
    let re = Regex::new(sensitive_keys_regex).map_err(TreblleError::Regex)?;

    Ok(match data {
        Value::Object(map) => {
            let mut new_map = Map::new();
            for (key, value) in map {
                if re.is_match(key) {
                    new_map.insert(key.clone(), Value::String("*****".to_string()));
                } else {
                    new_map.insert(key.clone(), mask_sensitive_data(value, sensitive_keys_regex)?);
                }
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => Value::Array(
            arr.iter()
                .map(|v| mask_sensitive_data(v, sensitive_keys_regex))
                .collect::<Result<Vec<_>>>()?,
        ),
        _ => data.clone(),
    })
}

/// Extract the IP address from a list of headers.
pub fn extract_ip_from_headers(headers: &[(String, String)]) -> Option<String> {
    headers
        .iter()
        .find(|(key, _)| {
            key.eq_ignore_ascii_case(header::FORWARDED.as_str()) ||
                key.eq_ignore_ascii_case("X-Forwarded-For") ||
                key.eq_ignore_ascii_case("X-Real-IP")
        })
        .and_then(|(_, value)| value.split(',').next())
        .map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_json() {
        assert!(is_json("application/json"));
        assert!(is_json("Application/JSON"));
        assert!(!is_json("text/plain"));
    }

    #[test]
    fn test_mask_sensitive_data() -> Result<()> {
        let data = json!({
            "username": "john_doe",
            "password": "secret123",
            "email": "john@example.com",
            "nested": {
                "credit_card": "1234-5678-9012-3456"
            }
        });

        let masked = mask_sensitive_data(&data, r"password|credit_card")?;

        assert_eq!(masked["username"], "john_doe");
        assert_eq!(masked["password"], "*****");
        assert_eq!(masked["email"], "john@example.com");
        assert_eq!(masked["nested"]["credit_card"], "*****");

        Ok(())
    }

    #[test]
    fn test_extract_ip_from_headers() {
        let headers = vec![
            (header::FORWARDED.to_string(), "for=192.168.1.1".to_string()),
            ("X-Forwarded-For".to_string(), "10.0.0.1, 10.0.0.2".to_string()),
            ("X-Real-IP".to_string(), "172.16.0.1".to_string()),
        ];

        assert_eq!(extract_ip_from_headers(&headers), Some("192.168.1.1".to_string()));

        let headers = vec![
            ("X-Forwarded-For".to_string(), "10.0.0.1, 10.0.0.2".to_string()),
        ];

        assert_eq!(extract_ip_from_headers(&headers), Some("10.0.0.1".to_string()));

        let headers = vec![
            ("X-Real-IP".to_string(), "172.16.0.1".to_string()),
        ];

        assert_eq!(extract_ip_from_headers(&headers), Some("172.16.0.1".to_string()));

        let headers = vec![
            (header::USER_AGENT.to_string(), "Mozilla".to_string()),
        ];

        assert_eq!(extract_ip_from_headers(&headers), None);
    }
}