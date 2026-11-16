//! The pricing calculator page (#54) at `/calculator/`. The files in
//! `web/calculator/` are built into the binary, so a deployment serves the
//! same page the repository has without copying anything else.

use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Router,
};

/// Path under `/calculator/`, content type and bytes.
const FILES: [(&str, &str, &[u8]); 5] = [
    (
        "index.html",
        "text/html; charset=utf-8",
        include_bytes!("../../../web/calculator/index.html"),
    ),
    (
        "app.js",
        "text/javascript; charset=utf-8",
        include_bytes!("../../../web/calculator/app.js"),
    ),
    (
        "style.css",
        "text/css; charset=utf-8",
        include_bytes!("../../../web/calculator/style.css"),
    ),
    (
        "pkg/satchel_calc.js",
        "text/javascript; charset=utf-8",
        include_bytes!("../../../web/calculator/pkg/satchel_calc.js"),
    ),
    (
        "pkg/satchel_calc_bg.wasm",
        "application/wasm",
        include_bytes!("../../../web/calculator/pkg/satchel_calc_bg.wasm"),
    ),
];

fn file(path: &str) -> Response {
    match FILES.iter().find(|(p, _, _)| *p == path) {
        Some((_, kind, body)) => ([(header::CONTENT_TYPE, *kind)], *body).into_response(),
        None => (StatusCode::NOT_FOUND, "no such calculator file").into_response(),
    }
}

/// `/calculator` redirects to `/calculator/`, so the page's relative links
/// work the same here and on GitHub Pages.
pub fn router() -> Router {
    Router::new()
        .route(
            "/calculator",
            get(|| async { Redirect::permanent("/calculator/") }),
        )
        .route("/calculator/", get(|| async { file("index.html") }))
        .route(
            "/calculator/{*path}",
            get(|Path(path): Path<String>| async move { file(&path) }),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    async fn fetch(uri: &str) -> (StatusCode, Option<String>, Vec<u8>) {
        let response = router()
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let kind = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_owned());
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec();
        (status, kind, body)
    }

    #[tokio::test]
    async fn serves_the_page() {
        let (status, kind, body) = fetch("/calculator/").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(kind.as_deref(), Some("text/html; charset=utf-8"));
        assert!(String::from_utf8(body)
            .unwrap()
            .contains("Satchel cost story"));
    }

    #[tokio::test]
    async fn redirects_the_bare_path() {
        let (status, _, _) = fetch("/calculator").await;
        assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    }

    #[tokio::test]
    async fn serves_the_wasm_with_its_type() {
        let (status, kind, body) = fetch("/calculator/pkg/satchel_calc_bg.wasm").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(kind.as_deref(), Some("application/wasm"));
        assert_eq!(&body[..4], b"\0asm");
    }

    #[tokio::test]
    async fn every_asset_the_page_loads_exists() {
        for path in ["app.js", "style.css", "pkg/satchel_calc.js"] {
            let (status, _, _) = fetch(&format!("/calculator/{path}")).await;
            assert_eq!(status, StatusCode::OK, "{path}");
        }
        let (status, _, _) = fetch("/calculator/nope.js").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
