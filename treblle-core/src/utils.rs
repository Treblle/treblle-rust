use http::header::HeaderMap;
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashSet;

/// Masks sensitive data in a JSON value based on both regex patterns and exact string matches.
/// For primitive values, returns a clone.
/// For objects and arrays, traverses them to mask sensitive fields.
pub fn mask_sensitive_data(
    data: &Value,
    patterns: &[Regex],
    exact_matches: &HashSet<String>,
) -> Value {
    match data {
        Value::Object(map) => {
            let mut new_map = Map::new();
            for (key, value) in map {
                let should_mask =
                    exact_matches.contains(key) || patterns.iter().any(|re| re.is_match(key));
                let new_value = if should_mask && !value.is_object() {
                    /* @TODO: Only mask leaf nodes or mask full objects? `&& !value.is_object()` */
                    Value::String("*****".to_string())
                } else {
                    mask_sensitive_data(value, patterns, exact_matches)
                };
                new_map.insert(key.clone(), new_value);
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => {
            let mut new_arr = Vec::with_capacity(arr.len());
            for value in arr {
                new_arr.push(mask_sensitive_data(value, patterns, exact_matches));
            }
            Value::Array(new_arr)
        }
        _ => data.clone(),
    }
}

/// Converts a HashMap to a JSON Value for masking
pub fn hashmap_to_json_value(map: &std::collections::HashMap<String, String>) -> Value {
    Value::Object(map.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect())
}

/// Converts a JSON Value back to a HashMap after masking
pub fn json_value_to_hashmap(value: Value) -> std::collections::HashMap<String, String> {
    match value {
        Value::Object(map) => map
            .into_iter()
            .filter_map(|(k, v)| match v {
                Value::String(s) => Some((k, s)),
                _ => None,
            })
            .collect(),
        _ => std::collections::HashMap::new(),
    }
}

/// Extract the IP address from request headers.
/// Checks common headers in order of preference:
/// 1. Forwarded (RFC 7239)
/// 2. X-Forwarded-For
/// 3. X-Real-IP
pub fn extract_ip_from_headers(headers: &HeaderMap) -> Option<String> {
    // Try Forwarded header first (RFC 7239)
    if let Some(forwarded) = headers.get(http::header::FORWARDED) {
        if let Ok(value) = forwarded.to_str() {
            if let Some(ip) = value
                .split(';')
                .find(|part| part.trim().to_lowercase().starts_with("for="))
                .and_then(|for_part| {
                    for_part.trim().trim_start_matches("for=").trim_matches('"').split(',').next()
                })
            {
                return Some(ip.trim().to_string());
            }
        }
    }

    // Try X-Forwarded-For
    if let Some(forwarded_for) = headers.get("x-forwarded-for") {
        if let Ok(value) = forwarded_for.to_str() {
            if let Some(ip) = value.split(',').next() {
                return Some(ip.trim().to_string());
            }
        }
    }

    // Try X-Real-IP
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip) = real_ip.to_str() {
            return Some(ip.trim().to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;
    use http::header::HeaderValue;
    use serde_json::json;

    pub fn test_field_masking(field: &str) -> bool {
        let config = Config::builder().api_key("test_api_key").build().unwrap();
        config.should_mask_field(field)
    }

    pub fn test_route_ignoring(route: &str) -> bool {
        let config = Config::builder().api_key("test_api_key").build().unwrap();
        config.should_ignore_route(route)
    }

    #[test]
    fn test_masked_fields_patterns() {
        // Authentication & Security
        assert!(test_field_masking("password"));
        assert!(test_field_masking("password_hash"));
        assert!(test_field_masking("auth_token"));
        assert!(test_field_masking("api_key"));
        assert!(test_field_masking("apikey_test"));
        assert!(test_field_masking("access_token_secret"));
        assert!(test_field_masking("private_key"));

        // Payment Information
        assert!(test_field_masking("card_number"));
        assert!(test_field_masking("cardnumber"));
        assert!(test_field_masking("cc_num"));
        assert!(test_field_masking("cvv"));
        assert!(test_field_masking("cvv2"));
        assert!(test_field_masking("pin_code"));
        assert!(test_field_masking("account_number"));

        // Personal Information
        assert!(test_field_masking("ssn"));
        assert!(test_field_masking("social_security_number"));
        assert!(test_field_masking("tax_id"));
        assert!(test_field_masking("passport_no"));
        assert!(test_field_masking("driver_license"));
        assert!(test_field_masking("birth_date"));
        assert!(test_field_masking("dob"));

        // Should NOT mask
        assert!(!test_field_masking("username"));
        assert!(!test_field_masking("first_name"));
        assert!(!test_field_masking("address"));
        assert!(!test_field_masking("public_key"));
    }

    #[test]
    fn test_ignored_routes_patterns() {
        // Health & Monitoring
        assert!(test_route_ignoring("/health"));
        assert!(test_route_ignoring("/health/check"));
        assert!(test_route_ignoring("/alive/status"));
        assert!(test_route_ignoring("/metrics"));
        assert!(test_route_ignoring("/prometheus/metrics"));

        // Debug & Development
        assert!(test_route_ignoring("/debug/users"));
        assert!(test_route_ignoring("/_debug/test"));
        assert!(test_route_ignoring("/dev/api"));

        // Admin & Internal
        assert!(test_route_ignoring("/admin/users"));
        assert!(test_route_ignoring("/internal/metrics"));
        assert!(test_route_ignoring("/_internal/debug"));

        // Documentation
        assert!(test_route_ignoring("/swagger/api"));
        assert!(test_route_ignoring("/openapi/spec"));
        assert!(test_route_ignoring("/docs/api"));

        // Should NOT ignore
        assert!(!test_route_ignoring("/api/users"));
        assert!(!test_route_ignoring("/v1/products"));
        assert!(!test_route_ignoring("/public/metrics"));
        assert!(!test_route_ignoring("/healthything")); // Avoid false positives
    }

    #[test]
    fn test_mask_sensitive_data() {
        let regex_patterns = vec![Regex::new(r"(?i)credit_card").unwrap()];
        let exact_matches: HashSet<String> =
            vec!["password".to_string(), "api_key".to_string()].into_iter().collect();

        let data = json!({
            "password": "secret123",
            "credit_card": "4111-1111-1111-1111",
            "api_key": "test_key",
            "user": {
                "password": "user_secret",
                "email": "test@example.com",
                "credit_card_number": "4111-1111-1111-1111"
            }
        });

        let masked = mask_sensitive_data(&data, &regex_patterns, &exact_matches);

        // Test exact matches
        assert_eq!(masked["password"], "*****");
        assert_eq!(masked["api_key"], "*****");
        assert_eq!(masked["user"]["password"], "*****");

        // Test regex patterns
        assert_eq!(masked["credit_card"], "*****");
        assert_eq!(masked["user"]["credit_card_number"], "*****");

        // Test unmasked fields
        assert_eq!(masked["user"]["email"], "test@example.com");
    }

    #[test]
    fn test_hashmap_conversion() {
        let mut map = std::collections::HashMap::new();
        map.insert("key1".to_string(), "value1".to_string());
        map.insert("password".to_string(), "secret".to_string());

        let patterns = vec![Regex::new(r"(?i)password").unwrap()];
        let exact_matches = HashSet::new();

        let json_value = hashmap_to_json_value(&map);
        let masked = mask_sensitive_data(&json_value, &patterns, &exact_matches);
        let result = json_value_to_hashmap(masked);

        assert_eq!(result["key1"], "value1");
        assert_eq!(result["password"], "*****");
    }

    #[test]
    fn test_extract_ip_from_headers() {
        let mut headers = HeaderMap::new();

        // Test Forwarded header
        headers.insert(
            http::header::FORWARDED,
            HeaderValue::from_static("for=192.0.2.60;proto=http;by=203.0.113.43"),
        );
        assert_eq!(extract_ip_from_headers(&headers), Some("192.0.2.60".to_string()));

        // Test X-Forwarded-For
        headers.clear();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.195, 2001:db8:85a3:8d3:1319:8a2e:370:7348"),
        );
        assert_eq!(extract_ip_from_headers(&headers), Some("203.0.113.195".to_string()));

        // Test X-Real-IP
        headers.clear();
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.195"));
        assert_eq!(extract_ip_from_headers(&headers), Some("203.0.113.195".to_string()));

        // Test with no IP headers
        headers.clear();
        headers.insert(http::header::USER_AGENT, HeaderValue::from_static("Mozilla/5.0"));
        assert_eq!(extract_ip_from_headers(&headers), None);
    }
}
