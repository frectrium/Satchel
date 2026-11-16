//! The files API in S3 mode, against a fake S3 server, with the admin login.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use satchel_files::{auth, s3::S3Store, AppState, Config, Limits};
use serde_json::Value;

type Objects = Arc<Mutex<BTreeMap<String, Bytes>>>;

async fn fake_list(
    State(objects): State<Objects>,
    Query(q): Query<BTreeMap<String, String>>,
) -> impl IntoResponse {
    let prefix = q.get("prefix").cloned().unwrap_or_default();
    let contents: String = objects
        .lock()
        .unwrap()
        .iter()
        .filter(|(k, _)| k.starts_with(&prefix))
        .map(|(k, v)| {
            format!(
                "<Contents><Key>{k}</Key><LastModified>2026-11-15T00:00:00.000Z</LastModified><ETag>\"e\"</ETag><Size>{}</Size><StorageClass>STANDARD</StorageClass></Contents>",
                v.len()
            )
        })
        .collect();
    (
        [(header::CONTENT_TYPE, "application/xml")],
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ListBucketResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\"><Name>b</Name><Prefix>{prefix}</Prefix><KeyCount>1</KeyCount><MaxKeys>1000</MaxKeys><IsTruncated>false</IsTruncated>{contents}</ListBucketResult>"
        ),
    )
}

async fn fake_put(
    State(objects): State<Objects>,
    Path((_, key)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> StatusCode {
    assert!(
        headers.get(header::CONTENT_LENGTH).is_some(),
        "S3 needs a Content-Length"
    );
    objects.lock().unwrap().insert(key, body);
    StatusCode::OK
}

async fn fake_get(
    State(objects): State<Objects>,
    Path((_, key)): Path<(String, String)>,
) -> Result<Bytes, StatusCode> {
    objects
        .lock()
        .unwrap()
        .get(&key)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)
}

async fn fake_delete(
    State(objects): State<Objects>,
    Path((_, key)): Path<(String, String)>,
) -> StatusCode {
    objects.lock().unwrap().remove(&key);
    StatusCode::NO_CONTENT
}

async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

struct Harness {
    url: String,
    objects: Objects,
    http: reqwest::Client,
}

const PASSWORD: &str = "a long test password";

async fn harness(limits: Limits) -> Harness {
    std::env::set_var("AWS_ACCESS_KEY_ID", "test");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
    std::env::remove_var("AWS_SESSION_TOKEN");
    let objects: Objects = Arc::default();
    let s3 = serve(
        Router::new()
            .route("/{bucket}", get(fake_list))
            .route("/{bucket}/", get(fake_list))
            .route(
                "/{bucket}/{*key}",
                get(fake_get).put(fake_put).delete(fake_delete),
            )
            .layer(axum::extract::DefaultBodyLimit::disable())
            .with_state(objects.clone()),
    )
    .await;
    let store = S3Store::new("bucket", "ap-south-1", Some(&s3)).unwrap();
    let admins = format!("admin:{}", auth::hash_password(PASSWORD));
    let state = AppState::s3(
        store,
        auth::Auth::parse(&admins, false).unwrap(),
        Config {
            allowed_origins: vec![],
            limits,
        },
    );
    Harness {
        url: serve(satchel_files::router(state)).await,
        objects,
        http: reqwest::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
    }
}

impl Harness {
    async fn login(&self, password: &str) -> u16 {
        self.http
            .post(format!("{}/v1/login", self.url))
            .json(&serde_json::json!({"user": "admin", "password": password}))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    }

    async fn upload(&self, name: &str, data: &[u8]) -> (u16, Value) {
        let r = self
            .http
            .post(format!("{}/v1/files?name={name}", self.url))
            .body(data.to_vec())
            .send()
            .await
            .unwrap();
        let status = r.status().as_u16();
        (status, r.json().await.unwrap_or(Value::Null))
    }
}

#[tokio::test]
async fn everything_but_info_needs_a_login() {
    let h = harness(Limits::default()).await;
    let info: Value = h
        .http
        .get(format!("{}/v1/info", h.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(info["backend"], "s3");
    assert_eq!(info["login_required"], true);

    for path in [
        "/v1/files",
        "/v1/stats",
        "/v1/files/1",
        "/v1/files/1/content",
    ] {
        let r = h.http.get(format!("{}{path}", h.url)).send().await.unwrap();
        assert_eq!(r.status(), 401, "{path}");
    }
    let (status, _) = h.upload("x", b"abc").await;
    assert_eq!(status, 401);
    assert!(h.objects.lock().unwrap().is_empty());

    assert_eq!(h.login("wrong password").await, 401);
    assert_eq!(h.login(PASSWORD).await, 200);
    let me: Value = h
        .http
        .get(format!("{}/v1/me", h.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["user"], "admin");
}

#[tokio::test]
async fn repeated_wrong_passwords_are_slowed_down() {
    let h = harness(Limits::default()).await;
    for _ in 0..3 {
        assert_eq!(h.login("wrong password").await, 401);
    }
    // The next attempt has to wait, even with the right password.
    let r = h
        .http
        .post(format!("{}/v1/login", h.url))
        .json(&serde_json::json!({"user": "admin", "password": PASSWORD}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 429);
    assert!(r.headers().contains_key("retry-after"));
}

#[tokio::test]
async fn a_file_is_one_whole_object() {
    let h = harness(Limits::default()).await;
    h.login(PASSWORD).await;
    let data = vec![7u8; 9 * 1024 * 1024];
    let (status, report) = h.upload("video.mp4", &data).await;
    assert_eq!(status, 201, "{report}");
    assert_eq!(report["backend"], "s3");
    assert_eq!(report["objects"], 1);
    assert_eq!(report["bytes"], data.len() as u64);
    {
        let objects = h.objects.lock().unwrap();
        assert_eq!(objects.len(), 1);
        let (key, stored) = objects.iter().next().unwrap();
        assert!(
            key.starts_with("files/") && key.ends_with("/video.mp4"),
            "{key}"
        );
        assert_eq!(stored.len(), data.len());
    }

    let files: Value = h
        .http
        .get(format!("{}/v1/files", h.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(files[0]["name"], "video.mp4");
    assert_eq!(files[0]["size"], data.len() as u64);
    let id = files[0]["id"].as_i64().unwrap();

    let r = h
        .http
        .get(format!("{}/v1/files/{id}/content", h.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 303);
    let location = r.headers()["location"].to_str().unwrap();
    assert!(location.contains("X-Amz-Signature"), "{location}");
    assert!(
        location.contains("response-content-disposition"),
        "{location}"
    );

    let stats: Value = h
        .http
        .get(format!("{}/v1/stats", h.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stats["files"], 1);
    assert_eq!(stats["logical_bytes"], data.len() as u64);
    assert!(stats["raw_bytes"].is_null());

    let r = h
        .http
        .delete(format!("{}/v1/files/{id}", h.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert!(h.objects.lock().unwrap().is_empty());
}

#[tokio::test]
async fn folder_paths_survive_in_s3_keys() {
    let h = harness(Limits::default()).await;
    h.login(PASSWORD).await;
    let (status, report) = h.upload("trip%2F2024%2Fbeach.jpg", b"sand").await;
    assert_eq!(status, 201, "{report}");
    let key = h.objects.lock().unwrap().keys().next().unwrap().clone();
    assert!(key.ends_with("/trip/2024/beach.jpg"), "{key}");
    let files: Value = h
        .http
        .get(format!("{}/v1/files", h.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(files[0]["name"], "trip/2024/beach.jpg");
}

#[tokio::test]
async fn limits_hold_for_s3() {
    let h = harness(Limits {
        max_file_bytes: Some(1_000),
        max_total_bytes: Some(1_500),
    })
    .await;
    h.login(PASSWORD).await;
    assert_eq!(h.upload("big", &[1u8; 1_001]).await.0, 413);
    let (s, r) = h.upload("ok", &[1u8; 1_000]).await;
    assert_eq!(s, 201, "{r}");
    assert_eq!(h.upload("full", &[1u8; 600]).await.0, 507);
    assert_eq!(h.objects.lock().unwrap().len(), 1);
}
