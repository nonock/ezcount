//! The pages the relay serves: the join page an invite link opens, and the web version of
//! the app.

use axum::http::{header, HeaderValue};
use axum::response::IntoResponse;
use axum::Router;
use std::path::Path as FsPath;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

/// What the web version may load and talk to: only this origin, plus running its WebAssembly.
const WEB_CSP: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval';      style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self';      connect-src 'self'; worker-src 'self'; object-src 'none'; base-uri 'none';      form-action 'none'; frame-ancestors 'none'";

/// The web version's files. Vite names everything in `assets/` by its content, so those are
/// cached for good; the rest (`index.html`, `theme.js`) is checked on every load.
pub(crate) fn web_app(dir: &FsPath) -> Router {
    let cache = |value: &'static str| {
        SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, HeaderValue::from_static(value))
    };
    let assets = Router::new()
        .fallback_service(ServeDir::new(dir.join("assets")))
        .layer(cache("public, max-age=31536000, immutable"));
    let rest = Router::new()
        .fallback_service(ServeDir::new(dir))
        .layer(cache("no-cache"));
    Router::new()
        .nest_service("/assets", assets)
        .fallback_service(rest)
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(WEB_CSP),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
}

pub(crate) async fn join_page(has_web: bool) -> impl IntoResponse {
    let page = include_str!("join.html");
    // Tells the page to offer the web version too.
    let page = if has_web {
        page.replacen("<body>", "<body data-web>", 1)
    } else {
        page.to_string()
    };
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
                 base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
            ),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        page,
    )
}
