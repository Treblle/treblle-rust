use crate::{
    schema::{RequestInfo, ResponseInfo},
    ErrorInfo, ServerInfo,
};
use std::time::Duration;

pub trait TreblleExtractor {
    type Request;
    type Response;

    fn extract_request_info(req: &Self::Request) -> RequestInfo;
    fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo;
    fn extract_error_info(res: &Self::Response) -> Option<Vec<ErrorInfo>>;
    fn extract_server_info() -> ServerInfo;
}
