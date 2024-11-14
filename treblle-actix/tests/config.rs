use actix_web::{test, web, App, FromRequest};
use treblle_actix::{ActixConfig, Treblle, TreblleConfig};

#[actix_web::test]
async fn test_actix_config() {
    let config = ActixConfig::builder()
        .api_key("test_key")
        .project_id("test_project")
        .buffer_response(true)
        .build()
        .unwrap();

    assert_eq!(config.core.api_key, "test_key");
    assert_eq!(config.core.project_id, "test_project");
    assert!(config.buffer_response);
}

#[actix_web::test]
async fn test_treblle_builder() {
    let treblle = Treblle::new("api_key".to_string());

    assert_eq!(treblle.config.core.api_key, "api_key");
    assert!(treblle.config.core.masked_fields.iter().any(|r| r.as_str().contains("password")));
    assert!(treblle.config.core.ignored_routes.iter().any(|r| r.as_str().contains("/health")));
}

#[actix_web::test]
async fn test_treblle_config_extraction() {
    let config =
        ActixConfig::builder().api_key("test_key").project_id("test_project").build().unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(config.clone()))
            .route("/test", web::get().to(|| async { "ok" })),
    )
    .await;

    let req = test::TestRequest::default().to_request();

    let srv_req = test::call_service(&app, req).await;
    let config_extracted = TreblleConfig::extract(&srv_req.request()).await.unwrap();

    assert_eq!(config_extracted.0.core.api_key, config.core.api_key);
    assert_eq!(config_extracted.0.core.project_id, config.core.project_id);
}
