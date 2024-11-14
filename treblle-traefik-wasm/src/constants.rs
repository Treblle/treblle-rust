//! Constant values specific to the Traefik WASM plugin.

/// Log levels
pub mod log_level {
    pub const DEBUG: i32 = -1;
    pub const INFO: i32 = 0;
    pub const WARN: i32 = 1;
    pub const ERROR: i32 = 2;
    pub const NONE: i32 = 3;
}

/// HTTP-related constants specific to WASM
pub mod http {
    pub const REQUEST_KIND: u32 = 0;
    pub const RESPONSE_KIND: u32 = 1;
    pub const MAX_RETRIES: usize = 3;
    pub const MAX_POOL_SIZE: usize = 10;
}

pub mod host_features {
    pub const FEATURE_BUFFER_REQUEST: u32 = 1;
    pub const FEATURE_BUFFER_RESPONSE: u32 = 2;
    pub const FEATURE_TRAILERS: u32 = 4;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(log_level::DEBUG, -1);
        assert_eq!(http::REQUEST_KIND, 0);
        assert_eq!(http::RESPONSE_KIND, 1);
    }
}
