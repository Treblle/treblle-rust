# Treblle Core

[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]

[crates-badge]: https://img.shields.io/crates/v/treblle-core.svg
[crates-url]: https://crates.io/crates/treblle-core
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/Treblle/treblle-rust/blob/main/LICENSE

Core functionality for Treblle SDK implementations in Rust. This crate provides the foundation for building Treblle middleware across different Rust web frameworks.

## Features

- 🔒 Sensitive data masking
- 🚦 Route blacklisting
- 📦 Framework-agnostic design
- 🔄 Request/Response monitoring
- 🎯 Fire-and-forget API integration
- ⚡ High-performance processing

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
treblle-core = "0.1.0"
```

## Example

```rust
use treblle_core::{Config, TreblleClient, HttpExtractor};

// Create configuration
let config = Config::new(
    "your-api-key".to_string(),
    "your-project-id".to_string(),
);

// Initialize client
let client = TreblleClient::new(config)?;

// Implement the HttpExtractor trait for your framework
struct MyFrameworkExtractor;

impl HttpExtractor for MyFrameworkExtractor {
    type Request = MyRequest;
    type Response = MyResponse;

    fn extract_request_info(req: &Self::Request) -> RequestInfo {
        // Implementation details
    }

    fn extract_response_info(res: &Self::Response, duration: Duration) -> ResponseInfo {
        // Implementation details
    }

    fn extract_error_info(res: &Self::Response) -> Option<Vec<ErrorInfo>> {
        // Implementation details
    }
}
```

## Framework Support

This core library is used by the following official Treblle middleware implementations:

- [treblle-axum](https://crates.io/crates/treblle-axum)
- [treblle-actix](https://crates.io/crates/treblle-actix)
- [treblle-rocket](https://crates.io/crates/treblle-rocket)
- [treblle-traefik-wasm](https://crates.io/crates/treblle-traefik-wasm)

## Configuration

### Custom Masking Patterns

```rust
let mut config = Config::new("api-key".to_string(), "project-id".to_string());
config.add_masked_fields(vec!["custom_secret.*".to_string()])?;
```

### Route Blacklisting

```rust
let mut config = Config::new("api-key".to_string(), "project-id".to_string());
config.add_ignored_routes(vec!["/health.*".to_string()])?;
```

### Custom API URLs

```rust
let mut config = Config::new("api-key".to_string(), "project-id".to_string());
config.set_api_urls(vec!["https://custom.treblle.com".to_string()]);
```

## Safety and Performance

- Zero-cost abstractions for request/response processing
- Non-blocking, fire-and-forget design
- Failsafe error handling that never impacts the main application
- Efficient JSON processing and memory management

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](../CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.

## Support

- [Official Documentation](https://docs.treblle.com)
- [Discord](https://treblle.com/chat)
- [GitHub Issues](https://github.com/Treblle/treblle-rust/issues)

## Security

For security issues, please email security@treblle.com instead of using the issue tracker.