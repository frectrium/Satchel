//! Where blocks go: Satchel's block API, or S3 for the baseline.

use std::time::Duration;

use satchel_common::BlockHash;

/// A store the load generator can put blocks into and get them back from.
pub enum Backend {
    /// `PUT` and `GET /blocks/{hash}` on `satchel-api` (DDL-002).
    Satchel {
        client: reqwest::Client,
        url: String,
    },
    /// One S3 object per block, keyed by its hash (exercise plan, section 6).
    #[cfg(feature = "s3")]
    S3 {
        client: reqwest::Client,
        bucket: rusty_s3::Bucket,
        credentials: rusty_s3::Credentials,
    },
}

fn client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?)
}

impl Backend {
    pub fn satchel(url: &str) -> anyhow::Result<Self> {
        Ok(Self::Satchel {
            client: client()?,
            url: url.trim_end_matches('/').to_owned(),
        })
    }

    /// S3 in `region`. Credentials come from `AWS_ACCESS_KEY_ID`,
    /// `AWS_SECRET_ACCESS_KEY` and, if set, `AWS_SESSION_TOKEN`. A custom
    /// `endpoint` uses path-style URLs, for tests.
    #[cfg(feature = "s3")]
    pub fn s3(bucket: &str, region: &str, endpoint: Option<&str>) -> anyhow::Result<Self> {
        use anyhow::Context;
        use rusty_s3::{Bucket, Credentials, UrlStyle};

        let (url, style) = match endpoint {
            Some(e) => (e.to_owned(), UrlStyle::Path),
            None => (
                format!("https://s3.{region}.amazonaws.com"),
                UrlStyle::VirtualHost,
            ),
        };
        let key = std::env::var("AWS_ACCESS_KEY_ID").context("AWS_ACCESS_KEY_ID is not set")?;
        let secret =
            std::env::var("AWS_SECRET_ACCESS_KEY").context("AWS_SECRET_ACCESS_KEY is not set")?;
        let credentials = match std::env::var("AWS_SESSION_TOKEN") {
            Ok(token) => Credentials::new_with_token(key, secret, token),
            Err(_) => Credentials::new(key, secret),
        };
        let bucket = Bucket::new(url.parse()?, style, bucket.to_owned(), region.to_owned())?;
        Ok(Self::S3 {
            client: client()?,
            bucket,
            credentials,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Satchel { .. } => "satchel",
            #[cfg(feature = "s3")]
            Self::S3 { .. } => "s3",
        }
    }

    /// Stores a block. Errors describe what went wrong, for the error count.
    pub async fn put(&self, hash: &BlockHash, block: Vec<u8>) -> Result<(), String> {
        let request = match self {
            Self::Satchel { client, url } => client.put(format!("{url}/blocks/{hash}")),
            #[cfg(feature = "s3")]
            Self::S3 {
                client,
                bucket,
                credentials,
            } => {
                use rusty_s3::S3Action;
                let url = bucket
                    .put_object(Some(credentials), &hash.to_string())
                    .sign(Duration::from_secs(3600));
                client.put(url)
            }
        };
        let response = request
            .body(block)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        match response.status().as_u16() {
            200 | 201 => Ok(()),
            status => Err(format!("put returned HTTP {status}")),
        }
    }

    /// Reads a block back and checks it hashes to `hash`. Returns its size.
    pub async fn get(&self, hash: &BlockHash) -> Result<usize, String> {
        let request = match self {
            Self::Satchel { client, url } => client.get(format!("{url}/blocks/{hash}")),
            #[cfg(feature = "s3")]
            Self::S3 {
                client,
                bucket,
                credentials,
            } => {
                use rusty_s3::S3Action;
                let url = bucket
                    .get_object(Some(credentials), &hash.to_string())
                    .sign(Duration::from_secs(3600));
                client.get(url)
            }
        };
        let response = request.send().await.map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(format!("get returned HTTP {status}"));
        }
        let body = response.bytes().await.map_err(|e| e.to_string())?;
        if !hash.verify(&body) {
            return Err("get returned bytes that don't match the hash".to_owned());
        }
        Ok(body.len())
    }
}
