use treblle_axum::{AxumConfig, Treblle};

#[test]
fn test_treblle_builder() {
    let treblle = Treblle::new("api_key");

    // Only test api_key as it's the only required field
    assert_eq!(treblle.config.core.api_key, "api_key");
}

#[test]
fn test_axum_config() {
    let config = AxumConfig::builder()
        .api_key("test_key")
        .add_masked_fields(vec!["password"])
        .add_ignored_routes(vec!["/health"])
        .build()
        .unwrap();

    assert_eq!(config.core.api_key, "test_key");
    assert!(config.core.should_mask_field("password"));
    assert!(config.core.should_ignore_route("/health"));
}
