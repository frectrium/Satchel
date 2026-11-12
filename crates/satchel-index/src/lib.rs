//! Satchel Block Index and Replication Table (#66): JSON over HTTP on one
//! SQLite database.
//!
//! Magic Pocket counterpart: the Block Index, a sharded MySQL cluster behind
//! an RPC layer, and the Replication Table [B2]. Decision Log: DDL-003.

pub mod store;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use satchel_common::cluster::{BlockEntry, Volume, VolumeState};
use satchel_common::BlockHash;
use serde::{Deserialize, Serialize};

pub use store::Store;
use store::{PutBlock, UpdateVolume};

/// One row of `GET /blocks?volume=`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedBlock {
    pub hash: BlockHash,
    #[serde(flatten)]
    pub entry: BlockEntry,
}

#[derive(Deserialize)]
struct NewVolume {
    nodes: Vec<String>,
}

#[derive(Serialize)]
struct StaleGeneration {
    error: &'static str,
    current: u64,
}

type AppState = Arc<Store>;

/// The index's routes, without `/health`.
pub fn router(store: Store) -> Router {
    Router::new()
        .route(
            "/blocks/{hash}",
            get(get_block).put(put_block).delete(delete_block),
        )
        .route("/blocks", get(list_blocks))
        .route("/volumes", get(list_volumes).post(create_volume))
        .route("/volumes/{id}", get(get_volume).put(update_volume))
        .with_state(Arc::new(store))
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, message.into()).into_response()
}

fn internal(e: impl std::fmt::Display) -> Response {
    tracing::error!("index store: {e}");
    error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// Runs a store call off the async workers, since SQLite blocks.
async fn blocking<T: Send + 'static>(
    store: &AppState,
    f: impl FnOnce(&Store) -> rusqlite::Result<T> + Send + 'static,
) -> Result<T, Response> {
    let store = store.clone();
    tokio::task::spawn_blocking(move || f(&store))
        .await
        .map_err(internal)?
        .map_err(internal)
}

fn bad_hash() -> Response {
    error(
        StatusCode::BAD_REQUEST,
        "the hash must be 64 hex characters",
    )
}

async fn get_block(State(s): State<AppState>, Path(raw): Path<String>) -> Response {
    let Ok(hash) = raw.parse::<BlockHash>() else {
        return bad_hash();
    };
    match blocking(&s, move |st| st.get_block(&hash)).await {
        Ok(Some(e)) => Json(e).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "no such block"),
        Err(r) => r,
    }
}

async fn put_block(
    State(s): State<AppState>,
    Path(raw): Path<String>,
    Json(entry): Json<BlockEntry>,
) -> Response {
    let Ok(hash) = raw.parse::<BlockHash>() else {
        return bad_hash();
    };
    if entry.length == 0 {
        return error(StatusCode::BAD_REQUEST, "empty blocks are not stored");
    }
    match blocking(&s, move |st| st.put_block(&hash, &entry)).await {
        Ok(PutBlock::Created) => (StatusCode::CREATED, Json(entry)).into_response(),
        Ok(PutBlock::Exists(existing)) => (StatusCode::OK, Json(existing)).into_response(),
        Ok(PutBlock::NoVolume) => error(
            StatusCode::BAD_REQUEST,
            format!("no volume {}", entry.volume),
        ),
        Ok(PutBlock::VolumeNotOpen) => error(
            StatusCode::CONFLICT,
            format!("volume {} is not open", entry.volume),
        ),
        Err(r) => r,
    }
}

async fn delete_block(State(s): State<AppState>, Path(raw): Path<String>) -> Response {
    let Ok(hash) = raw.parse::<BlockHash>() else {
        return bad_hash();
    };
    match blocking(&s, move |st| st.delete_block(&hash)).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => error(StatusCode::NOT_FOUND, "no such block"),
        Err(r) => r,
    }
}

async fn list_blocks(
    State(s): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let Some(Ok(volume)) = q.get("volume").map(|v| v.parse::<u64>()) else {
        return error(StatusCode::BAD_REQUEST, "use /blocks?volume={id}");
    };
    match blocking(&s, move |st| st.blocks_in_volume(volume)).await {
        Ok(rows) => Json(
            rows.into_iter()
                .map(|(hash, entry)| ListedBlock { hash, entry })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(r) => r,
    }
}

async fn create_volume(State(s): State<AppState>, Json(body): Json<NewVolume>) -> Response {
    let unique: HashSet<&String> = body.nodes.iter().collect();
    if body.nodes.is_empty() || unique.len() != body.nodes.len() {
        return error(
            StatusCode::BAD_REQUEST,
            "nodes must be a non-empty list of different node ids",
        );
    }
    match blocking(&s, move |st| st.create_volume(&body.nodes)).await {
        Ok(v) => (StatusCode::CREATED, Json(v)).into_response(),
        Err(r) => r,
    }
}

async fn get_volume(State(s): State<AppState>, Path(id): Path<u64>) -> Response {
    match blocking(&s, move |st| st.get_volume(id)).await {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "no such volume"),
        Err(r) => r,
    }
}

async fn list_volumes(
    State(s): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let state = match q.get("state").map(String::as_str) {
        None => None,
        Some("open") => Some(VolumeState::Open),
        Some("closed") => Some(VolumeState::Closed),
        Some(other) => {
            return error(
                StatusCode::BAD_REQUEST,
                format!("state must be open or closed, not {other:?}"),
            )
        }
    };
    match blocking(&s, move |st| st.volumes(state)).await {
        Ok(v) => Json(v).into_response(),
        Err(r) => r,
    }
}

async fn update_volume(
    State(s): State<AppState>,
    Path(id): Path<u64>,
    Json(next): Json<Volume>,
) -> Response {
    if next.id != id {
        return error(StatusCode::BAD_REQUEST, "the body's id must match the path");
    }
    if next.nodes.is_empty() {
        return error(StatusCode::BAD_REQUEST, "a volume needs at least one node");
    }
    match blocking(&s, move |st| st.update_volume(&next)).await {
        Ok(UpdateVolume::Updated(v)) => Json(v).into_response(),
        Ok(UpdateVolume::NotFound) => error(StatusCode::NOT_FOUND, "no such volume"),
        Ok(UpdateVolume::Stale { current }) => (
            StatusCode::CONFLICT,
            Json(StaleGeneration {
                error: "generation must be the current one plus 1",
                current,
            }),
        )
            .into_response(),
        Err(r) => r,
    }
}
