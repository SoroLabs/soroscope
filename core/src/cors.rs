//! CORS layer assembly for the public HTTP API (issue #670).
//!
//! The frontend calls the API cross-origin, so every public route — including
//! error responses and the OPTIONS preflight — must carry the
//! `Access-Control-Allow-Origin` family of headers. The layer is built here so
//! it can be tested in isolation from the full server assembly in `main.rs`.

use axum::http::{header, HeaderValue, Method};
use std::time::Duration;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// How long browsers may cache a preflight response.
const PREFLIGHT_MAX_AGE: Duration = Duration::from_secs(3600);

/// Build the CORS layer applied to the public API router.
///
/// `allowed_origins` is a comma-separated list of origins, e.g.
/// `"https://app.example.com,https://staging.example.com"`. When the list is
/// empty (the default) every origin is allowed — a permissive fallback meant
/// for development. The wildcard cannot be combined with credentialed browser
/// requests, so deployments that authenticate from the browser should set an
/// explicit origin list via the `CORS_ALLOWED_ORIGINS` environment variable.
pub fn build_cors_layer(allowed_origins: &str) -> CorsLayer {
    let origins: Vec<HeaderValue> = allowed_origins
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .filter_map(|origin| match HeaderValue::from_str(origin) {
            Ok(value) => Some(value),
            Err(_) => {
                tracing::warn!(origin, "Ignoring invalid CORS origin");
                None
            }
        })
        .collect();

    let allow_origin = if origins.is_empty() {
        tracing::warn!(
            "CORS_ALLOWED_ORIGINS is not set; allowing any origin (development fallback)"
        );
        AllowOrigin::any()
    } else {
        AllowOrigin::list(origins)
    };

    CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .max_age(PREFLIGHT_MAX_AGE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{header, Method, Request, StatusCode};
    use axum::response::Response;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    use header::ACCESS_CONTROL_ALLOW_ORIGIN as ACAO;

    fn app(layer: CorsLayer) -> Router {
        Router::new()
            .route("/ok", get(|| async { "ok" }).post(|| async { "ok" }))
            .route(
                "/fail",
                get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") }),
            )
            .layer(layer)
    }

    async fn send(app: Router, req: Request<Body>) -> Response {
        app.oneshot(req).await.expect("request should not error")
    }

    fn get_with_origin(path: &str, origin: &str) -> Request<Body> {
        Request::builder()
            .method(Method::GET)
            .uri(path)
            .header(header::ORIGIN, origin)
            .body(Body::empty())
            .unwrap()
    }

    fn preflight(path: &str, origin: &str) -> Request<Body> {
        Request::builder()
            .method(Method::OPTIONS)
            .uri(path)
            .header(header::ORIGIN, origin)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(
                header::ACCESS_CONTROL_REQUEST_HEADERS,
                "content-type,authorization",
            )
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn permissive_default_allows_any_origin_on_success() {
        let app = app(build_cors_layer(""));
        let resp = send(app, get_with_origin("/ok", "https://app.example")).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers().get(ACAO).unwrap(), "*");
    }

    #[tokio::test]
    async fn error_responses_carry_allow_origin() {
        let app = app(build_cors_layer(""));
        let resp = send(app, get_with_origin("/fail", "https://app.example")).await;

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            resp.headers().get(ACAO).is_some(),
            "error responses must include Access-Control-Allow-Origin, \
             otherwise the browser hides the real status from the frontend"
        );
    }

    #[tokio::test]
    async fn preflight_grants_json_post_with_auth_header() {
        let app = app(build_cors_layer(""));
        let resp = send(app, preflight("/ok", "https://app.example")).await;

        assert!(
            resp.headers().get(ACAO).is_some(),
            "preflight response must include Access-Control-Allow-Origin"
        );
        let methods = resp
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_METHODS)
            .expect("preflight must advertise allowed methods")
            .to_str()
            .unwrap()
            .to_ascii_uppercase();
        assert!(methods.contains("POST"), "POST missing from: {methods}");
        assert!(methods.contains("GET"), "GET missing from: {methods}");
        let headers = resp
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
            .expect("preflight must advertise allowed headers")
            .to_str()
            .unwrap()
            .to_ascii_lowercase();
        assert!(
            headers.contains("content-type"),
            "content-type missing from: {headers}"
        );
        assert!(
            headers.contains("authorization"),
            "authorization missing from: {headers}"
        );
    }

    #[tokio::test]
    async fn configured_origin_is_echoed_back() {
        let app = app(build_cors_layer("https://scope.example"));
        let resp = send(app, get_with_origin("/ok", "https://scope.example")).await;

        assert_eq!(resp.headers().get(ACAO).unwrap(), "https://scope.example");
    }

    #[tokio::test]
    async fn unlisted_origin_is_not_allowed() {
        let app = app(build_cors_layer(
            "https://scope.example,https://staging.scope.example",
        ));
        let resp = send(app, get_with_origin("/ok", "https://evil.example")).await;

        assert!(
            resp.headers().get(ACAO).is_none(),
            "origins outside the configured list must not be granted access"
        );
    }
}
