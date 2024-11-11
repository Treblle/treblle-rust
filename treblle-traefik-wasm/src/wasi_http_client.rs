use std::io::Write;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use crate::certs::load_root_certs;
use crate::log_error;
use lazy_static::lazy_static;
use rustls::{ClientConfig, ClientConnection, RootCertStore, ServerName, StreamOwned};
use treblle_core::constants::http::REQUEST_TIMEOUT;
use treblle_core::TreblleError;
use url::Url;
use wasmedge_wasi_socket::TcpStream;

type TlsStream = StreamOwned<ClientConnection, TcpStream>;

lazy_static! {
    static ref TLS_CONFIG: Mutex<Option<Arc<ClientConfig>>> = Mutex::new(None);
}

/// A connection pool entry
struct PooledConnection {
    stream: TlsStream,
    last_used: Instant,
    host: String,
}

/// WASI-compatible HTTP client for sending data to Treblle API
pub struct WasiHttpClient {
    api_urls: Vec<String>,
    current_url_index: AtomicUsize,
    connection_pool: Mutex<Vec<PooledConnection>>,
    max_retries: usize,
    max_pool_size: usize,
}

impl WasiHttpClient {
    /// Creates a new WasiHttpClient instance
    pub fn new(api_urls: Vec<String>, max_retries: usize, max_pool_size: usize) -> Self {
        Self {
            api_urls,
            current_url_index: AtomicUsize::new(0),
            connection_pool: Mutex::new(Vec::new()),
            max_retries,
            max_pool_size,
        }
    }

    /// Sends data to the Treblle API with retries
    pub fn send(&self, payload: &[u8], api_key: &str) -> Result<(), TreblleError> {
        let mut retries = 0;
        let mut last_error = None;

        while retries < self.max_retries {
            match self.try_send(payload, api_key) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    log_error!("Failed to send data (attempt {}): {}", retries + 1, e);
                    last_error = Some(e);
                    retries += 1;
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| TreblleError::Http("Maximum retry attempts exceeded".to_string())))
    }

    /// Attempts to send data to the Treblle API once
    fn try_send(&self, payload: &[u8], api_key: &str) -> Result<(), TreblleError> {
        let url = self.get_next_url()?;
        let parsed_url = Url::parse(&url).map_err(|e| TreblleError::InvalidUrl(e.to_string()))?;

        let host = parsed_url
            .host_str()
            .ok_or_else(|| TreblleError::InvalidUrl("No host in URL".to_string()))?
            .to_string();

        let port = parsed_url
            .port_or_known_default()
            .ok_or_else(|| TreblleError::InvalidUrl("Invalid port".to_string()))?;

        let mut stream = self.get_connection(&host, port)?;

        let request = self.build_request(&host, parsed_url.path(), payload, api_key);

        self.write_request(&mut stream, &request, payload)?;

        // We don't need to read the response, but we should try to reuse the connection
        self.return_connection(stream, host);

        Ok(())
    }

    /// Gets the next URL from the rotation
    fn get_next_url(&self) -> Result<String, TreblleError> {
        let urls = &self.api_urls;
        if urls.is_empty() {
            return Err(TreblleError::Config("No API URLs configured".to_string()));
        }

        let index = self.current_url_index.fetch_add(1, Ordering::SeqCst) % urls.len();
        Ok(urls[index].clone())
    }

    /// Gets an active connection from the pool or creates a new one
    fn get_connection(&self, host: &str, port: u16) -> Result<TlsStream, TreblleError> {
        // Try to reuse an existing connection
        if let Some(conn) = self.get_pooled_connection(host) {
            return Ok(conn);
        }

        // Create a new connection
        let stream =
            TcpStream::connect((host, port)).map_err(|e| TreblleError::Tcp(e.to_string()))?;

        stream.set_nonblocking(true).map_err(|e| TreblleError::Tcp(e.to_string()))?;

        let server_name = ServerName::try_from(host)
            .map_err(|_| TreblleError::InvalidHostname(host.to_string()))?;

        let tls_config = self.get_tls_config()?;

        let client = ClientConnection::new(tls_config, server_name).map_err(TreblleError::Tls)?;

        Ok(StreamOwned::new(client, stream))
    }

    /// Attempts to get a connection from the pool
    fn get_pooled_connection(&self, host: &str) -> Option<TlsStream> {
        let mut pool = self.connection_pool.lock().ok()?;

        // Find and remove a matching connection that isn't too old
        if let Some(index) = pool.iter().position(|conn| {
            conn.host == host && conn.last_used.elapsed() < Duration::from_secs(60)
        }) {
            Some(pool.swap_remove(index).stream)
        } else {
            None
        }
    }

    /// Returns a connection to the pool if there's space
    fn return_connection(&self, stream: TlsStream, host: String) {
        if let Ok(mut pool) = self.connection_pool.lock() {
            if pool.len() < self.max_pool_size {
                pool.push(PooledConnection { stream, last_used: Instant::now(), host });
            }
        }
    }

    /// Gets or initializes the TLS configuration
    fn get_tls_config(&self) -> Result<Arc<ClientConfig>, TreblleError> {
        let mut config_guard =
            TLS_CONFIG.lock().map_err(|e| TreblleError::LockError(e.to_string()))?;

        if let Some(config) = config_guard.as_ref() {
            return Ok(config.clone());
        }

        let mut root_store = RootCertStore::empty();
        load_root_certs(&mut root_store)?;

        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        let config = Arc::new(config);
        *config_guard = Some(config.clone());

        Ok(config)
    }

    /// Builds the HTTP request string
    fn build_request(&self, host: &str, path: &str, payload: &[u8], api_key: &str) -> String {
        format!(
            "POST {} HTTP/1.1\r\n\
             Host: {}\r\n\
             Content-Type: application/json\r\n\
             X-Api-Key: {}\r\n\
             Content-Length: {}\r\n\
             Connection: keep-alive\r\n\
             \r\n",
            path,
            host,
            api_key,
            payload.len()
        )
    }

    /// Writes the request and payload to the stream with timeout handling
    fn write_request(
        &self,
        stream: &mut TlsStream,
        request: &str,
        payload: &[u8],
    ) -> Result<(), TreblleError> {
        let start = Instant::now();
        let mut request_bytes = request.as_bytes().to_vec();
        request_bytes.extend_from_slice(payload);

        let mut written = 0;
        while written < request_bytes.len() {
            match stream.write(&request_bytes[written..]) {
                Ok(n) => written += n,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if start.elapsed() > REQUEST_TIMEOUT {
                        return Err(TreblleError::Timeout);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => return Err(TreblleError::Io(e)),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_rotation() {
        let client = WasiHttpClient::new(
            vec!["https://api1.treblle.com".to_string(), "https://api2.treblle.com".to_string()],
            3,
            10,
        );

        let url1 = client.get_next_url().unwrap();
        let url2 = client.get_next_url().unwrap();
        let url3 = client.get_next_url().unwrap();

        assert_eq!(url1, "https://api1.treblle.com");
        assert_eq!(url2, "https://api2.treblle.com");
        assert_eq!(url3, "https://api1.treblle.com");
    }

    #[test]
    fn test_build_request() {
        let client = WasiHttpClient::new(vec!["https://api.treblle.com".to_string()], 3, 10);
        let payload = b"test";
        let request = client.build_request("api.treblle.com", "/v1/logs", payload, "test-key");

        assert!(request.contains("POST /v1/logs HTTP/1.1"));
        assert!(request.contains("Host: api.treblle.com"));
        assert!(request.contains("X-Api-Key: test-key"));
        assert!(request.contains("Content-Length: 4"));
    }

    #[test]
    fn test_empty_urls() {
        let client = WasiHttpClient::new(vec![], 3, 10);
        assert!(client.get_next_url().is_err());
    }
}
