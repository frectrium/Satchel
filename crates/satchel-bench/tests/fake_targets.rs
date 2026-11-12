//! Runs the load generator against in-process fakes of Satchel's block API
//! and of S3, over real HTTP.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    routing::put,
    Router,
};
use satchel_bench::{run, Backend, Config, Mix, Op};
use satchel_common::BlockHash;

type Store = Arc<Mutex<HashMap<String, Bytes>>>;

/// `PUT` and `GET /blocks/{hash}`, checking the hash like the real API.
fn fake_satchel(fail_puts: bool) -> Router {
    async fn put_block(
        State((store, fail)): State<(Store, bool)>,
        Path(hash): Path<String>,
        body: Bytes,
    ) -> StatusCode {
        if fail {
            return StatusCode::SERVICE_UNAVAILABLE;
        }
        let ok = hash.parse::<BlockHash>().is_ok_and(|h| h.verify(&body));
        if !ok {
            return StatusCode::BAD_REQUEST;
        }
        store.lock().unwrap().insert(hash, body);
        StatusCode::CREATED
    }
    async fn get_block(
        State((store, _)): State<(Store, bool)>,
        Path(hash): Path<String>,
    ) -> Result<Bytes, StatusCode> {
        store
            .lock()
            .unwrap()
            .get(&hash)
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)
    }
    Router::new()
        .route("/blocks/{hash}", put(put_block).get(get_block))
        .with_state((Store::default(), fail_puts))
}

async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

fn config(mix: &str, max_ops: u64) -> Config {
    Config {
        block_size: 4096,
        mix: Mix::parse(mix).unwrap(),
        clients: 4,
        warmup: Duration::ZERO,
        duration: Duration::from_secs(10),
        max_ops,
        prefill_bytes: 64 * 1024,
        seed: 7,
    }
}

#[tokio::test]
async fn measures_puts_and_gets_against_satchel() {
    let url = serve(fake_satchel(false)).await;
    let report = run(Backend::satchel(&url).unwrap(), config("2:1", 300))
        .await
        .unwrap();
    assert_eq!(report.target, "satchel");
    let (puts, gets) = (&report.ops[&Op::Put], &report.ops[&Op::Get]);
    assert_eq!(puts.errors + gets.errors, 0, "{:?}", report.error_examples);
    let total = puts.samples + gets.samples;
    assert!((300..=303).contains(&total), "{total}");
    // 2 gets per put.
    assert!(gets.samples > puts.samples * 3 / 2, "{puts:?} {gets:?}");
    assert!(puts.p50_ms <= puts.p99_ms && puts.p99_ms <= puts.max_ms);
    assert!(puts.mib_per_sec > 0.0);
    // The JSON result has the fields the plotting script reads.
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["config"]["block_size"], 4096);
    assert!(json["ops"]["get"]["p999_ms"].is_number());
}

#[tokio::test]
async fn counts_failed_puts_as_errors() {
    let url = serve(fake_satchel(true)).await;
    let mut cfg = config("0:1", 40);
    cfg.prefill_bytes = 0;
    let report = run(Backend::satchel(&url).unwrap(), cfg).await.unwrap();
    let puts = &report.ops[&Op::Put];
    assert_eq!(puts.samples, 0);
    assert!(puts.errors >= 40);
    assert_eq!(report.error_examples, ["put returned HTTP 503"]);
}

#[tokio::test]
async fn prefill_failure_stops_the_run() {
    let url = serve(fake_satchel(true)).await;
    let err = run(Backend::satchel(&url).unwrap(), config("1:0", 10))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("prefill"), "{err}");
}

/// Path-style S3: `PUT` and `GET /{bucket}/{key}`. Signatures aren't checked,
/// but they must be there.
#[cfg(feature = "s3")]
#[tokio::test]
async fn measures_s3_with_signed_requests() {
    use axum::extract::RawQuery;

    async fn put_object(
        State(store): State<Store>,
        Path((bucket, key)): Path<(String, String)>,
        RawQuery(query): RawQuery,
        body: Bytes,
    ) -> StatusCode {
        if bucket != "satchel-baseline" || !query.unwrap_or_default().contains("X-Amz-Signature") {
            return StatusCode::FORBIDDEN;
        }
        store.lock().unwrap().insert(key, body);
        StatusCode::OK
    }
    async fn get_object(
        State(store): State<Store>,
        Path((_, key)): Path<(String, String)>,
    ) -> Result<Bytes, StatusCode> {
        store
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)
    }
    let app = Router::new()
        .route("/{bucket}/{key}", put(put_object).get(get_object))
        .with_state(Store::default());
    let url = serve(app).await;

    // Only this test reads them.
    std::env::set_var("AWS_ACCESS_KEY_ID", "test-key");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "test-secret");
    let backend = Backend::s3("satchel-baseline", "ap-south-1", Some(&url)).unwrap();
    let report = run(backend, config("2:1", 60)).await.unwrap();
    assert_eq!(report.target, "s3");
    assert_eq!(report.ops[&Op::Put].errors + report.ops[&Op::Get].errors, 0);
}
