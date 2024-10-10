use crate::schema::{RequestInfo, ResponseInfo};
use std::time::Duration;

pub trait TreblleExtractor {
    type Request;
    type Response;

    fn extract_request_info(req: &Self::Request) -> RequestInfo;
    fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo;
    fn extract_request_body(req: &Self::Request) -> Option<String>;
    fn extract_response_body(res: &Self::Response) -> Option<String>;
}