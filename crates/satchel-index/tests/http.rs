//! The index API from #66 over real HTTP, the way the frontend, Master and
//! verifier call it.

use std::path::PathBuf;

use reqwest::StatusCode;
use satchel_common::cluster::{BlockEntry, Volume, VolumeKind, VolumeState};
use satchel_common::BlockHash;
use satchel_index::{router, ListedBlock, Store};
use serde_json::json;

async fn serve(store: Store) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router(store)).await.unwrap();
    });
    (url, task)
}

async fn index() -> String {
    serve(Store::open_in_memory().unwrap()).await.0
}

fn hash(n: u8) -> String {
    BlockHash::of(&[n]).to_string()
}

async fn open_volume(c: &reqwest::Client, url: &str) -> Volume {
    let r = c
        .post(format!("{url}/volumes"))
        .json(&json!({"nodes": ["node-z1-1", "node-z2-1", "node-z3-1"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    r.json().await.unwrap()
}

async fn put(c: &reqwest::Client, url: &str, hash: &str, e: BlockEntry) -> reqwest::Response {
    c.put(format!("{url}/blocks/{hash}"))
        .json(&e)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn ts_idx_1_insert_lookup_and_delete() {
    let (c, url) = (reqwest::Client::new(), index().await);
    let v = open_volume(&c, &url).await;
    let e = BlockEntry {
        volume: v.id,
        offset: 0,
        length: 2_097_152,
    };
    assert_eq!(
        put(&c, &url, &hash(1), e).await.status(),
        StatusCode::CREATED
    );
    let got: BlockEntry = c
        .get(format!("{url}/blocks/{}", hash(1)))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(got, e);
    let del = c
        .delete(format!("{url}/blocks/{}", hash(1)))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::NO_CONTENT);
    let again = c
        .delete(format!("{url}/blocks/{}", hash(1)))
        .send()
        .await
        .unwrap();
    assert_eq!(again.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn ts_idx_2_unknown_hash_is_a_404_not_an_error() {
    let (c, url) = (reqwest::Client::new(), index().await);
    let r = c
        .get(format!("{url}/blocks/{}", hash(9)))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
    let bad = c.get(format!("{url}/blocks/xyz")).send().await.unwrap();
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn ts_idx_3_second_put_of_a_hash_keeps_the_first_entry() {
    let (c, url) = (reqwest::Client::new(), index().await);
    let v = open_volume(&c, &url).await;
    let first = BlockEntry {
        volume: v.id,
        offset: 0,
        length: 10,
    };
    put(&c, &url, &hash(1), first).await;
    let second = put(
        &c,
        &url,
        &hash(1),
        BlockEntry {
            offset: 10,
            ..first
        },
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(second.json::<BlockEntry>().await.unwrap(), first);
}

#[tokio::test]
async fn ts_idx_4_entries_survive_an_index_restart() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ts-idx-4");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("index.sqlite3");
    let c = reqwest::Client::new();

    let (url, server) = serve(Store::open(&path).unwrap()).await;
    let v = open_volume(&c, &url).await;
    let e = BlockEntry {
        volume: v.id,
        offset: 0,
        length: 4096,
    };
    put(&c, &url, &hash(1), e).await;
    server.abort();
    let _ = server.await;

    let (url, _server) = serve(Store::open(&path).unwrap()).await;
    let got: BlockEntry = c
        .get(format!("{url}/blocks/{}", hash(1)))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(got, e);
    let vol: Volume = c
        .get(format!("{url}/volumes/{}", v.id))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(vol.size, 4096);
}

#[tokio::test]
async fn stale_volume_generation_gets_409() {
    let (c, url) = (reqwest::Client::new(), index().await);
    let v = open_volume(&c, &url).await;
    assert_eq!(v.generation, 1);
    let closed = Volume {
        state: VolumeState::Closed,
        generation: 2,
        ..v.clone()
    };
    let ok = c
        .put(format!("{url}/volumes/{}", v.id))
        .json(&closed)
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    let stale = c
        .put(format!("{url}/volumes/{}", v.id))
        .json(&closed)
        .send()
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let body: serde_json::Value = stale.json().await.unwrap();
    assert_eq!(body["current"], 2);
}

#[tokio::test]
async fn closed_volumes_take_no_new_blocks() {
    let (c, url) = (reqwest::Client::new(), index().await);
    let v = open_volume(&c, &url).await;
    let closed = Volume {
        state: VolumeState::Closed,
        generation: 2,
        ..v.clone()
    };
    c.put(format!("{url}/volumes/{}", v.id))
        .json(&closed)
        .send()
        .await
        .unwrap();
    let e = BlockEntry {
        volume: v.id,
        offset: 0,
        length: 1,
    };
    assert_eq!(
        put(&c, &url, &hash(1), e).await.status(),
        StatusCode::CONFLICT
    );
    let unknown = BlockEntry { volume: 99, ..e };
    assert_eq!(
        put(&c, &url, &hash(2), unknown).await.status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn lists_for_the_master_and_the_metadata_scan() {
    let (c, url) = (reqwest::Client::new(), index().await);
    let a = open_volume(&c, &url).await;
    let b = open_volume(&c, &url).await;
    let coded = Volume {
        state: VolumeState::Closed,
        kind: VolumeKind::Rs63,
        generation: 2,
        nodes: (1..=9).map(|n| format!("node-{n}")).collect(),
        ..b.clone()
    };
    c.put(format!("{url}/volumes/{}", b.id))
        .json(&coded)
        .send()
        .await
        .unwrap();
    let open: Vec<Volume> = c
        .get(format!("{url}/volumes?state=open"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(open.iter().map(|v| v.id).collect::<Vec<_>>(), [a.id]);
    let all: Vec<Volume> = c
        .get(format!("{url}/volumes"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].kind, VolumeKind::Rs63);

    for (n, offset) in [(1, 0), (2, 100)] {
        let e = BlockEntry {
            volume: a.id,
            offset,
            length: 100,
        };
        put(&c, &url, &hash(n), e).await;
    }
    let listed: Vec<ListedBlock> = c
        .get(format!("{url}/blocks?volume={}", a.id))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[1].entry.offset, 100);
    assert_eq!(listed[0].hash.to_string(), hash(1));
    let missing = c.get(format!("{url}/blocks")).send().await.unwrap();
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn bad_volume_requests_are_rejected() {
    let (c, url) = (reqwest::Client::new(), index().await);
    let empty = c
        .post(format!("{url}/volumes"))
        .json(&json!({"nodes": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(empty.status(), StatusCode::BAD_REQUEST);
    let twice = c
        .post(format!("{url}/volumes"))
        .json(&json!({"nodes": ["a", "a"]}))
        .send()
        .await
        .unwrap();
    assert_eq!(twice.status(), StatusCode::BAD_REQUEST);
    let v = open_volume(&c, &url).await;
    let wrong_id = Volume {
        id: v.id + 1,
        generation: 2,
        ..v.clone()
    };
    let r = c
        .put(format!("{url}/volumes/{}", v.id))
        .json(&wrong_id)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let none = c.get(format!("{url}/volumes/42")).send().await.unwrap();
    assert_eq!(none.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_frontend_client_works_against_the_real_index() {
    use satchel_api::clients::{http_client, CallError, IndexClient};
    let url = index().await;
    let v = open_volume(&reqwest::Client::new(), &url).await;
    let client = IndexClient {
        client: http_client(std::time::Duration::from_secs(5)),
        url,
    };
    let h = BlockHash::of(b"frontend");
    assert!(matches!(
        client.get_block(&h).await,
        Err(CallError::NotFound)
    ));
    let e = BlockEntry {
        volume: v.id,
        offset: 0,
        length: 8,
    };
    assert!(client.put_block(&h, &e).await.unwrap());
    assert!(!client.put_block(&h, &e).await.unwrap());
    assert_eq!(client.get_block(&h).await.unwrap(), e);
    assert_eq!(client.get_volume(v.id).await.unwrap().size, 8);
    let open = client.open_volumes().await.unwrap();
    assert_eq!(open.len(), 1);
    client.delete_block(&h).await.unwrap();
    assert!(matches!(
        client.get_block(&h).await,
        Err(CallError::NotFound)
    ));
}
