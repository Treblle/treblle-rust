use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Represents the main payload sent to Treblle API.
#[derive(Debug, Serialize, Deserialize)]
pub struct TrebllePayload {
    pub api_key: String,
    pub project_id: String,
    pub version: f32,
    pub sdk: String,
    pub data: PayloadData,
}

/// Contains the main data of the Treblle payload.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PayloadData {
    pub server: ServerInfo,
    pub language: LanguageInfo,
    pub request: RequestInfo,
    pub response: ResponseInfo,
    pub errors: Vec<ErrorInfo>,
}

/// Represents server information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerInfo {
    pub ip: String,
    pub timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub software: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    pub os: OsInfo,
}

/// Represents operating system information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OsInfo {
    pub name: String,
    pub release: String,
    pub architecture: String,
}

/// Represents programming language information.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LanguageInfo {
    pub name: String,
    pub version: String,
}

/// Represents HTTP request information.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RequestInfo {
    pub timestamp: DateTime<Utc>,
    pub ip: String,
    pub url: String,
    pub user_agent: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// Represents HTTP response information.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ResponseInfo {
    pub headers: HashMap<String, String>,
    pub code: u16,
    pub size: u64,
    pub load_time: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// Represents error information.
#[derive(Debug, Serialize, Deserialize, Clone)]  // Added Clone here
pub struct ErrorInfo {
    pub source: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
    pub file: String,
    pub line: u32,
}

impl TrebllePayload {
    pub fn new(api_key: String, project_id: String) -> Self {
        TrebllePayload {
            api_key,
            project_id,
            version: 0.1,
            sdk: format!("rust-core-{}", env!("CARGO_PKG_VERSION")),
            data: PayloadData::default(),
        }
    }

    pub fn to_json(&self) -> crate::Result<String> {
        serde_json::to_string(&self).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_treblle_payload_new() {
        let payload = TrebllePayload::new("test_key".to_string(), "test_project".to_string());
        assert_eq!(payload.api_key, "test_key");
        assert_eq!(payload.project_id, "test_project");
        assert_eq!(payload.version, 0.1);
        assert!(payload.sdk.starts_with("rust-core-"));
    }

    #[test]
    fn test_serialize_deserialize() {
        let payload = TrebllePayload::new("test_key".to_string(), "test_project".to_string());
        let serialized = serde_json::to_string(&payload).unwrap();
        let deserialized: TrebllePayload = serde_json::from_str(&serialized).unwrap();
        assert_eq!(payload.api_key, deserialized.api_key);
        assert_eq!(payload.project_id, deserialized.project_id);
    }

    #[test]
    fn test_error_info() {
        let error = ErrorInfo {
            source: "test".to_string(),
            error_type: "TestError".to_string(),
            message: "Test error message".to_string(),
            file: "test.rs".to_string(),
            line: 42,
        };

        let cloned = error.clone();
        assert_eq!(error.source, cloned.source);
        assert_eq!(error.error_type, cloned.error_type);
        assert_eq!(error.message, cloned.message);
        assert_eq!(error.file, cloned.file);
        assert_eq!(error.line, cloned.line);
    }
}