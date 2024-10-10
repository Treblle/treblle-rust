use crate::schema::{TrebllePayload, PayloadData, RequestInfo, ResponseInfo, ServerInfo, LanguageInfo};
use crate::Config;
use crate::utils::mask_sensitive_data;
use std::time::Duration;

pub trait HttpExtractor {
    type Request;
    type Response;

    fn extract_request_info(req: &Self::Request) -> RequestInfo;
    fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo;
}

pub struct PayloadBuilder;

impl PayloadBuilder {
    pub fn build_request_payload<E: HttpExtractor>(
        req: &E::Request,
        config: &Config,
    ) -> TrebllePayload {
        let mut request_info = E::extract_request_info(req);

        request_info.headers = mask_sensitive_data(&request_info.headers, &config.masked_fields);
        if let Some(body) = &request_info.body {
            request_info.body = Some(mask_sensitive_data(body, &config.masked_fields));
        }

        TrebllePayload {
            api_key: config.api_key.clone(),
            project_id: config.project_id.clone(),
            version: 0.1,
            sdk: format!("treblle-rust-{}", env!("CARGO_PKG_VERSION")),
            data: PayloadData {
                server: ServerInfo::default(),
                language: LanguageInfo {
                    name: "rust".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                request: request_info,
                response: ResponseInfo::default(),
                errors: Vec::new(),
            },
        }
    }

    pub fn build_response_payload<E: HttpExtractor>(
        res: &E::Response,
        config: &Config,
        duration: Duration,
    ) -> TrebllePayload {
        let mut response_info = E::extract_response_info(res, duration);

        response_info.headers = mask_sensitive_data(&response_info.headers, &config.masked_fields);
        if let Some(body) = &response_info.body {
            response_info.body = Some(mask_sensitive_data(body, &config.masked_fields));
        }

        TrebllePayload {
            api_key: config.api_key.clone(),
            project_id: config.project_id.clone(),
            version: 0.1,
            sdk: format!("treblle-rust-{}", env!("CARGO_PKG_VERSION")),
            data: PayloadData {
                server: ServerInfo::default(),
                language: LanguageInfo {
                    name: "rust".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                request: RequestInfo::default(),
                response: response_info,
                errors: Vec::new(),
            },
        }
    }
}