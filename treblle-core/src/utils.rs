use http::header::HeaderMap;
use regex::Regex;
use serde_json::{Map, Value};

/// Masks sensitive data in a JSON value based on regex patterns.
/// For primitive values, returns a clone.
/// For objects and arrays, traverses them to mask sensitive fields.
pub fn mask_sensitive_data(data: &Value, patterns: &[Regex]) -> Value {
    match data {
        Value::Object(map) => {
            let mut new_map = Map::new();
            for (key, value) in map {
                let should_mask = patterns.iter().any(|re| re.is_match(key));
                let new_value = if should_mask {
                    Value::String("*****".to_string())
                } else {
                    mask_sensitive_data(value, patterns)
                };
                new_map.insert(key.clone(), new_value);
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => {
            let mut new_arr = Vec::with_capacity(arr.len());
            for value in arr {
                new_arr.push(mask_sensitive_data(value, patterns));
            }
            Value::Array(new_arr)
        }
        // For primitive values, return a clone
        _ => data.clone(),
    }
}

/// Converts a HashMap to a JSON Value for masking
pub fn hashmap_to_json_value(map: &std::collections::HashMap<String, String>) -> Value {
    Value::Object(
        map.iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect(),
    )
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
///
/// Returns the first IP address found, or None if no IP address is found.
pub fn extract_ip_from_headers(headers: &HeaderMap) -> Option<String> {
    // Try Forwarded header first (RFC 7239)
    if let Some(forwarded) = headers.get(http::header::FORWARDED) {
        if let Ok(value) = forwarded.to_str() {
            if let Some(ip) = value
                .split(';')
                .find(|part| part.trim().to_lowercase().starts_with("for="))
                .and_then(|for_part| {
                    for_part
                        .trim()
                        .trim_start_matches("for=")
                        .trim_matches('"')
                        .split(',')
                        .next()
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
    use http::header::HeaderValue;
    use serde_json::json;

    #[test]
    fn test_mask_sensitive_data() {
        let patterns = vec![
            Regex::new(r"(?i)password").unwrap(),
            Regex::new(r"(?i)credit_card").unwrap(),
        ];

        let data = json!({
            "password": "secret123",
            "credit_card": "4111-1111-1111-1111",
            "user": {
                "password": "user_secret",
                "email": "test@example.com"
            }
        });

        let masked = mask_sensitive_data(&data, &patterns);

        assert_eq!(masked["password"], "*****");
        assert_eq!(masked["credit_card"], "*****");
        assert_eq!(masked["user"]["password"], "*****");
        assert_eq!(masked["user"]["email"], "test@example.com");
    }

    #[test]
    fn test_hashmap_conversion() {
        let mut map = std::collections::HashMap::new();
        map.insert("key1".to_string(), "value1".to_string());
        map.insert("password".to_string(), "secret".to_string());

        let patterns = vec![Regex::new(r"(?i)password").unwrap()];

        let json_value = hashmap_to_json_value(&map);
        let masked = mask_sensitive_data(&json_value, &patterns);
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
        assert_eq!(
            extract_ip_from_headers(&headers),
            Some("192.0.2.60".to_string())
        );

        // Test X-Forwarded-For
        headers.clear();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.195, 2001:db8:85a3:8d3:1319:8a2e:370:7348"),
        );
        assert_eq!(
            extract_ip_from_headers(&headers),
            Some("203.0.113.195".to_string())
        );

        // Test X-Real-IP
        headers.clear();
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.195"));
        assert_eq!(
            extract_ip_from_headers(&headers),
            Some("203.0.113.195".to_string())
        );

        // Test with no IP headers
        headers.clear();
        headers.insert(
            http::header::USER_AGENT,
            HeaderValue::from_static("Mozilla/5.0"),
        );
        assert_eq!(extract_ip_from_headers(&headers), None);
    }
}
