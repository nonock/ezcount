//! The pages the relay serves: the join page an invite link opens, what it says about
//! itself (privacy, deleting an account), and the web version of the app.

use axum::extract::State;
use axum::http::{header, HeaderValue};
use axum::response::IntoResponse;
use axum::Router;
use std::path::Path as FsPath;
use std::sync::Arc;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::Relay;

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

/// A page of the relay's own: it holds its style and its script, and loads nothing.
fn own_page(page: String) -> impl IntoResponse {
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

pub(crate) async fn join_page(has_web: bool) -> impl IntoResponse {
    let page = include_str!("join.html");
    // Tells the page to offer the web version too.
    let page = if has_web {
        page.replacen("<body>", "<body data-web>", 1)
    } else {
        page.to_string()
    };
    own_page(page)
}

/// `GET /privacy`: what the app and the relay keep about people, and who can read it.
pub(crate) async fn privacy_page(State(relay): State<Arc<Relay>>) -> impl IntoResponse {
    own_page(about_page(
        "Privacy policy · ezcount",
        include_str!("privacy.html"),
        &relay,
    ))
}

/// `GET /delete-account`: how to delete an account, and what goes with it.
pub(crate) async fn delete_account_page(State(relay): State<Arc<Relay>>) -> impl IntoResponse {
    own_page(about_page(
        "Delete your ezcount account",
        include_str!("delete-account.html"),
        &relay,
    ))
}

/// One of the pages the relay has about itself: `text` in their frame, for this relay.
fn about_page(title: &str, text: &str, relay: &Relay) -> String {
    framed(
        title,
        text,
        relay.contact.as_deref(),
        relay.web_dir.is_some(),
    )
}

/// `text` in the frame of the relay's pages (`page.html`). The page shows what suits the
/// relay: who to write to when it names a `contact`, and the way to the web version when it
/// serves one.
fn framed(title: &str, text: &str, contact: Option<&str>, has_web: bool) -> String {
    let mut body = String::from("<body");
    if contact.is_some() {
        body.push_str(" data-contact");
    }
    if has_web {
        body.push_str(" data-web");
    }
    body.push('>');
    include_str!("page.html")
        .replacen("<body>", &body, 1)
        .replacen("<!--title-->", &escape(title), 1)
        .replacen("<!--text-->", text, 1)
        .replace(
            "<!--contact-->",
            &contact.map(contact_link).unwrap_or_default(),
        )
}

/// Who runs the relay, as a link when it is an e-mail address or a page's.
fn contact_link(contact: &str) -> String {
    let text = escape(contact);
    let is_page = contact.starts_with("https://") || contact.starts_with("http://");
    let is_mail = !is_page && contact.contains('@') && !contact.contains(char::is_whitespace);
    if is_page {
        format!(r#"<a href="{text}">{text}</a>"#)
    } else if is_mail {
        format!(r#"<a href="mailto:{text}">{text}</a>"#)
    } else {
        text
    }
}

/// Text as HTML shows it, between tags or in an attribute.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests;
