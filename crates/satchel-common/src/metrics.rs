//! Prometheus metrics every Satchel service exposes on `GET /metrics` (#21).
//!
//! The experiments read latency, throughput and errors from here. Prometheus
//! adds the `service` and `zone` labels from its scrape config, so the metric
//! names are the same on every service. On Linux, the `process_*` metrics give
//! each service's CPU and memory.

use std::sync::LazyLock;
use std::time::Instant;

use axum::{
    extract::{MatchedPath, Request},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use prometheus::{
    exponential_buckets, register_histogram_vec, register_int_counter_vec, register_int_gauge,
    Encoder, HistogramVec, IntCounterVec, IntGauge, TextEncoder,
};

/// Route label for requests that matched no route, so raw paths never
/// become label values.
const UNMATCHED: &str = "unmatched";

static REQUESTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "satchel_http_requests_total",
        "HTTP requests handled, by method, route and status code.",
        &["method", "route", "status"]
    )
    .expect("metric is registered once")
});

static LATENCY: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "satchel_http_request_duration_seconds",
        "Time to handle an HTTP request, by method and route.",
        &["method", "route"],
        // 0.5 ms to about 16 s.
        exponential_buckets(0.0005, 2.0, 16).expect("valid buckets")
    )
    .expect("metric is registered once")
});

static NODE_STORED_BYTES: LazyLock<IntGauge> = LazyLock::new(|| {
    register_int_gauge!(
        "satchel_node_stored_bytes",
        "Bytes of block data and fragments a storage node holds on disk."
    )
    .expect("metric is registered once")
});

/// Bytes a storage node holds. Only storage nodes touch it, so only they
/// export it.
pub fn node_stored_bytes() -> &'static IntGauge {
    &NODE_STORED_BYTES
}

/// Adds `GET /metrics` to `app` and records every other request. Call it on
/// the full router, since a layer only covers routes added before it.
pub fn instrument(app: Router) -> Router {
    app.route("/metrics", get(render))
        .layer(middleware::from_fn(track))
}

async fn track(req: Request, next: Next) -> Response {
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map_or(UNMATCHED, MatchedPath::as_str)
        .to_owned();
    if route == "/metrics" {
        return next.run(req).await;
    }
    let method = req.method().as_str().to_owned();
    let start = Instant::now();
    let response = next.run(req).await;
    LATENCY
        .with_label_values(&[method.as_str(), route.as_str()])
        .observe(start.elapsed().as_secs_f64());
    REQUESTS
        .with_label_values(&[method.as_str(), route.as_str(), response.status().as_str()])
        .inc();
    response
}

async fn render() -> Response {
    let encoder = TextEncoder::new();
    match encoder.encode_to_string(&prometheus::gather()) {
        Ok(body) => (
            [(header::CONTENT_TYPE, encoder.format_type().to_owned())],
            body,
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use tower::ServiceExt;

    // Metrics are global, so each test uses its own routes.

    async fn get_text(app: &Router, path: &str) -> (StatusCode, String) {
        let response = app
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn labels_requests_by_route_not_by_path() {
        let app = instrument(Router::new().route("/test-route/{hash}", get(|| async { "ok" })));
        let (status, _) = get_text(&app, "/test-route/abc123").await;
        assert_eq!(status, StatusCode::OK);

        let (_, metrics) = get_text(&app, "/metrics").await;
        assert!(metrics.contains(
            r#"satchel_http_requests_total{method="GET",route="/test-route/{hash}",status="200"} 1"#
        ));
        assert!(metrics.contains(
            r#"satchel_http_request_duration_seconds_count{method="GET",route="/test-route/{hash}"} 1"#
        ));
        assert!(!metrics.contains("abc123"), "{metrics}");
    }

    #[tokio::test]
    async fn counts_errors_with_their_status() {
        let app = instrument(Router::new().route(
            "/test-error",
            get(|| async { StatusCode::SERVICE_UNAVAILABLE }),
        ));
        get_text(&app, "/test-error").await;
        let (_, metrics) = get_text(&app, "/metrics").await;
        assert!(metrics.contains(
            r#"satchel_http_requests_total{method="GET",route="/test-error",status="503"} 1"#
        ));
    }

    #[tokio::test]
    async fn unknown_paths_share_one_label() {
        let app = instrument(Router::new().route("/test-known", get(|| async { "ok" })));
        let (status, _) = get_text(&app, "/no-such-path-xyz").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (_, metrics) = get_text(&app, "/metrics").await;
        assert!(metrics.contains(r#"route="unmatched",status="404""#));
        assert!(!metrics.contains("no-such-path-xyz"), "{metrics}");
    }

    #[tokio::test]
    async fn metrics_endpoint_is_prometheus_text_and_not_counted() {
        let app = instrument(Router::new());
        let response = app
            .clone()
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/plain"));
        let (_, metrics) = get_text(&app, "/metrics").await;
        assert!(!metrics.contains(r#"route="/metrics""#), "{metrics}");
    }

    #[tokio::test]
    async fn node_gauge_is_exported_once_set() {
        node_stored_bytes().set(4096);
        let (_, metrics) = get_text(&instrument(Router::new()), "/metrics").await;
        assert!(metrics.contains("satchel_node_stored_bytes 4096"));
    }
}
