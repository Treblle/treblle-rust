use crate::schema::{TrebllePayload, PayloadData, ServerInfo, LanguageInfo, RequestInfo, ResponseInfo, ErrorInfo, OsInfo};
use crate::utils;
use chrono::Utc;
use std::collections::HashMap;

pub struct PayloadBuilder {
    payload: TrebllePayload,
}

impl PayloadBuilder {
    pub fn new(api_key: String, project_id: String) -> Self {
        PayloadBuilder {
            payload: TrebllePayload::new(api_key, project_id),
        }
    }

    pub fn with_data(mut self, data: PayloadData) -> Self {
        self.payload.data = data;
        self
    }

    pub fn server_info(mut self, ip: String, protocol: String) -> Self {
        self.payload.data.server = ServerInfo {
            ip,
            timezone: Utc::now().format("%Z").to_string(),
            protocol,
            os: OsInfo {
                name: std::env::consts::OS.to_string(),
                release: std::env::consts::FAMILY.to_string(),
                architecture: std::env::consts::ARCH.to_string(),
            },
            ..Default::default()
        };
        self
    }

    pub fn language_info(mut self, name: String, version: String) -> Self {
        self.payload.data.language = LanguageInfo { name, version };
        self
    }

    pub fn request_info(mut self, request: RequestInfo) -> Self {
        self.payload.data.request = request;
        self
    }

    pub fn response_info(mut self, response: ResponseInfo) -> Self {
        self.payload.data.response = response;
        self
    }

    pub fn add_error(mut self, error: ErrorInfo) -> Self {
        self.payload.data.errors.push(error);
        self
    }

    pub fn build(self) -> TrebllePayload {
        self.payload
    }
}

pub fn mask_payload(payload: &mut TrebllePayload, masked_fields: &[String]) {
    let sensitive_keys_regex = masked_fields.join("|");
    if let Some(body) = &mut payload.data.request.body {
        if let Ok(masked) = utils::mask_sensitive_data(body, &sensitive_keys_regex) {
            *body = masked;
        }
    }
    if let Some(body) = &mut payload.data.response.body {
        if let Ok(masked) = utils::mask_sensitive_data(body, &sensitive_keys_regex) {
            *body = masked;
        }
    }
    payload.data.request.headers = mask_headers(&payload.data.request.headers, masked_fields);
    payload.data.response.headers = mask_headers(&payload.data.response.headers, masked_fields);
}

fn mask_headers(headers: &HashMap<String, String>, masked_fields: &[String]) -> HashMap<String, String> {
    headers
        .iter()
        .map(|(k, v)| {
            if masked_fields.iter().any(|field| k.to_lowercase().contains(&field.to_lowercase())) {
                (k.clone(), "*****".to_string())
            } else {
                (k.clone(), v.clone())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_payload_builder() {
        let payload = PayloadBuilder::new("test_key".to_string(), "test_project".to_string())
            .server_info("127.0.0.1".to_string(), "http".to_string())
            .language_info("rust".to_string(), "1.55.0".to_string())
            .request_info(RequestInfo {
                timestamp: Utc::now(),
                ip: "192.168.1.1".to_string(),
                url: "https://api.example.com/test".to_string(),
                user_agent: "Mozilla/5.0".to_string(),
                method: "GET".to_string(),
                headers: HashMap::new(),
                body: None,
            })
            .response_info(ResponseInfo {
                headers: HashMap::new(),
                code: 200,
                size: 1024,
                load_time: 0.1,
                body: None,
            })
            .build();

        assert_eq!(payload.api_key, "test_key");
        assert_eq!(payload.project_id, "test_project");
        assert_eq!(payload.data.server.ip, "127.0.0.1");
        assert_eq!(payload.data.language.name, "rust");
        assert_eq!(payload.data.request.ip, "192.168.1.1");
        assert_eq!(payload.data.response.code, 200);
    }

    #[test]
    fn test_mask_payload() {
        let mut payload = PayloadBuilder::new("test_key".to_string(), "test_project".to_string())
            .request_info(RequestInfo {
                timestamp: Utc::now(),
                ip: "192.168.1.1".to_string(),
                url: "https://api.example.com/test".to_string(),
                user_agent: "Mozilla/5.0".to_string(),
                method: "POST".to_string(),
                headers: {
                    let mut map = HashMap::new();
                    map.insert("Authorization".to_string(), "Bearer token123".to_string());
                    map
                },
                body: Some(json!({
                    "username": "john_doe",
                    "password": "secret123"
                })),
            })
            .build();

        mask_payload(&mut payload, &["password".to_string(), "Authorization".to_string()]);

        assert_eq!(
            payload.data.request.headers.get("Authorization").unwrap(),
            "*****"
        );
        assert_eq!(
            payload.data.request.body.as_ref().unwrap()["password"],
            "*****"
        );
        assert_eq!(
            payload.data.request.body.as_ref().unwrap()["username"],
            "john_doe"
        );
    }
}