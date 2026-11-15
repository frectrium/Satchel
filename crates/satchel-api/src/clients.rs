//! HTTP clients for the Block Index (#66) and the storage nodes (#67).

use std::time::Duration;

use satchel_common::cluster::{BlockEntry, NodeAddr, Volume};
use satchel_common::BlockHash;

/// Why a call to another service failed.
#[derive(Debug)]
pub enum CallError {
    /// The service answered 404.
    NotFound,
    /// Unreachable, timed out, or answered with something unexpected.
    Failed(String),
}

impl CallError {
    fn from_status(what: &str, status: reqwest::StatusCode) -> Self {
        if status == reqwest::StatusCode::NOT_FOUND {
            Self::NotFound
        } else {
            Self::Failed(format!("{what} returned HTTP {status}"))
        }
    }
}

fn failed(what: &str) -> impl Fn(reqwest::Error) -> CallError + '_ {
    move |e| CallError::Failed(format!("{what}: {e}"))
}

pub fn http_client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .expect("HTTP client builds")
}

#[derive(Clone)]
pub struct IndexClient {
    pub client: reqwest::Client,
    pub url: String,
}

impl IndexClient {
    pub async fn get_block(&self, hash: &BlockHash) -> Result<BlockEntry, CallError> {
        let what = "index lookup";
        let response = self
            .client
            .get(format!("{}/blocks/{hash}", self.url))
            .send()
            .await
            .map_err(failed(what))?;
        if !response.status().is_success() {
            return Err(CallError::from_status(what, response.status()));
        }
        response.json().await.map_err(failed(what))
    }

    /// Records a block. Returns false if the hash was already there.
    pub async fn put_block(&self, hash: &BlockHash, entry: &BlockEntry) -> Result<bool, CallError> {
        let what = "index insert";
        let response = self
            .client
            .put(format!("{}/blocks/{hash}", self.url))
            .json(entry)
            .send()
            .await
            .map_err(failed(what))?;
        match response.status().as_u16() {
            201 => Ok(true),
            200 => Ok(false),
            _ => Err(CallError::from_status(what, response.status())),
        }
    }

    pub async fn delete_block(&self, hash: &BlockHash) -> Result<(), CallError> {
        let what = "index delete";
        let response = self
            .client
            .delete(format!("{}/blocks/{hash}", self.url))
            .send()
            .await
            .map_err(failed(what))?;
        if !response.status().is_success() {
            return Err(CallError::from_status(what, response.status()));
        }
        Ok(())
    }

    pub async fn get_volume(&self, id: u64) -> Result<Volume, CallError> {
        let what = "volume lookup";
        let response = self
            .client
            .get(format!("{}/volumes/{id}", self.url))
            .send()
            .await
            .map_err(failed(what))?;
        if !response.status().is_success() {
            return Err(CallError::from_status(what, response.status()));
        }
        response.json().await.map_err(failed(what))
    }

    pub async fn open_volumes(&self) -> Result<Vec<Volume>, CallError> {
        let what = "open volume list";
        let response = self
            .client
            .get(format!("{}/volumes?state=open", self.url))
            .send()
            .await
            .map_err(failed(what))?;
        if !response.status().is_success() {
            return Err(CallError::from_status(what, response.status()));
        }
        response.json().await.map_err(failed(what))
    }
}

#[derive(Clone)]
pub struct NodeClient {
    pub client: reqwest::Client,
}

#[derive(serde::Deserialize)]
struct Appended {
    offset: u64,
    length: u64,
}

impl NodeClient {
    /// Appends a block to `node`'s replica of `volume`. Returns its offset.
    pub async fn append(
        &self,
        node: &NodeAddr,
        volume: u64,
        hash: &BlockHash,
        block: axum::body::Bytes,
    ) -> Result<u64, CallError> {
        let what = format!("append to {}", node.id);
        let length = block.len() as u64;
        let response = self
            .client
            .post(format!("{}/extents/{volume}", node.url))
            .header("x-satchel-block", hash.to_string())
            .body(block)
            .send()
            .await
            .map_err(failed(&what))?;
        if response.status() != reqwest::StatusCode::CREATED {
            return Err(CallError::from_status(&what, response.status()));
        }
        let appended: Appended = response.json().await.map_err(failed(&what))?;
        if appended.length != length {
            return Err(CallError::Failed(format!(
                "{what}: stored {} bytes, sent {length}",
                appended.length
            )));
        }
        Ok(appended.offset)
    }

    /// Reads `length` bytes at `offset` of `node`'s replica of `volume`.
    pub async fn read(
        &self,
        node: &NodeAddr,
        volume: u64,
        offset: u64,
        length: u64,
    ) -> Result<axum::body::Bytes, CallError> {
        let what = format!("read from {}", node.id);
        let response = self
            .client
            .get(format!(
                "{}/extents/{volume}?offset={offset}&length={length}",
                node.url
            ))
            .send()
            .await
            .map_err(failed(&what))?;
        if !response.status().is_success() {
            return Err(CallError::from_status(&what, response.status()));
        }
        response.bytes().await.map_err(failed(&what))
    }
}
