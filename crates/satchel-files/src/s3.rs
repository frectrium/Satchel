//! The S3 backend: what Dropbox moved away from, for the website's "rent it"
//! side. Each file is one whole object at `files/{id}/{name}`, so S3 itself
//! is the file list and the service keeps no state of its own.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use axum::body::Bytes;
use futures_util::{Stream, StreamExt};
use rusty_s3::{actions::ListObjectsV2, Bucket, Credentials, S3Action, UrlStyle};
use sha2::{Digest, Sha256};

use crate::manifest::FileRecord;

const PREFIX: &str = "files/";
const SIGN_FOR: Duration = Duration::from_secs(300);

enum Creds {
    Static(Credentials),
    /// The instance role, from IMDSv2, refreshed every 30 minutes.
    Instance {
        imds: String,
        cached: Mutex<Option<(Credentials, Instant)>>,
    },
}

pub struct S3Store {
    http: reqwest::Client,
    bucket: Bucket,
    creds: Creds,
}

impl S3Store {
    /// `endpoint` is for tests (path-style URLs). Credentials come from
    /// `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` and `AWS_SESSION_TOKEN`
    /// when set, otherwise from the EC2 instance role.
    pub fn new(bucket: &str, region: &str, endpoint: Option<&str>) -> anyhow::Result<Self> {
        let (url, style) = match endpoint {
            Some(e) => (e.to_owned(), UrlStyle::Path),
            None => (
                format!("https://s3.{region}.amazonaws.com"),
                UrlStyle::VirtualHost,
            ),
        };
        let bucket = Bucket::new(url.parse()?, style, bucket.to_owned(), region.to_owned())?;
        let creds = match (
            std::env::var("AWS_ACCESS_KEY_ID"),
            std::env::var("AWS_SECRET_ACCESS_KEY"),
        ) {
            (Ok(key), Ok(secret)) => Creds::Static(match std::env::var("AWS_SESSION_TOKEN") {
                Ok(token) => Credentials::new_with_token(key, secret, token),
                Err(_) => Credentials::new(key, secret),
            }),
            _ => Creds::Instance {
                imds: std::env::var("SATCHEL_IMDS_URL")
                    .unwrap_or_else(|_| "http://169.254.169.254".into()),
                cached: Mutex::new(None),
            },
        };
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(600))
                .build()?,
            bucket,
            creds,
        })
    }

    async fn credentials(&self) -> anyhow::Result<Credentials> {
        match &self.creds {
            Creds::Static(c) => Ok(c.clone()),
            Creds::Instance { imds, cached } => {
                if let Some((c, at)) = cached.lock().expect("lock").as_ref() {
                    if at.elapsed() < Duration::from_secs(1800) {
                        return Ok(c.clone());
                    }
                }
                let fresh = instance_credentials(&self.http, imds).await?;
                *cached.lock().expect("lock") = Some((fresh.clone(), Instant::now()));
                Ok(fresh)
            }
        }
    }

    /// Every file, newest first.
    pub async fn list(&self) -> anyhow::Result<Vec<FileRecord>> {
        self.list_prefix(PREFIX).await
    }

    async fn list_prefix(&self, prefix: &str) -> anyhow::Result<Vec<FileRecord>> {
        let creds = self.credentials().await?;
        let mut files = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut action = self.bucket.list_objects_v2(Some(&creds));
            action.with_prefix(prefix);
            if let Some(t) = &token {
                action.with_continuation_token(t.clone());
            }
            let url = action.sign(SIGN_FOR);
            let r = self.http.get(url).send().await?;
            if !r.status().is_success() {
                bail!(
                    "list: HTTP {}: {}",
                    r.status(),
                    r.text().await.unwrap_or_default()
                );
            }
            let page = ListObjectsV2::parse_response(&r.text().await?)
                .map_err(|e| anyhow::anyhow!("list: {e}"))?;
            for obj in page.contents {
                if let Some(f) = record_from_key(&obj.key, obj.size.unwrap_or(0)) {
                    files.push(f);
                }
            }
            match page.next_continuation_token {
                Some(t) => token = Some(t),
                None => break,
            }
        }
        files.sort_by_key(|f| std::cmp::Reverse(f.id));
        Ok(files)
    }

    /// The object key and record for a file id.
    pub async fn find(&self, id: i64) -> anyhow::Result<Option<(String, FileRecord)>> {
        Ok(self
            .list_prefix(&format!("{PREFIX}{id}/"))
            .await?
            .into_iter()
            .next()
            .map(|f| (key_for(f.id, &f.name), f)))
    }

    /// Streams `size` bytes into a new object. Returns the record and the
    /// SHA-256 of what was sent.
    pub async fn put<S>(&self, name: &str, size: u64, body: S) -> anyhow::Result<FileRecord>
    where
        S: Stream<Item = Result<Bytes, axum::Error>> + Send + 'static,
    {
        let id = new_id();
        let key = key_for(id, name);
        let creds = self.credentials().await?;
        let url = self.bucket.put_object(Some(&creds), &key).sign(SIGN_FOR);
        let hasher = Arc::new(Mutex::new((Sha256::new(), 0u64)));
        let h = hasher.clone();
        let counted = body.map(move |chunk| {
            let chunk = chunk.map_err(std::io::Error::other)?;
            let mut g = h.lock().expect("lock");
            g.0.update(&chunk);
            g.1 += chunk.len() as u64;
            Ok::<_, std::io::Error>(chunk)
        });
        let r = self
            .http
            .put(url)
            .header(reqwest::header::CONTENT_LENGTH, size)
            .body(reqwest::Body::wrap_stream(counted))
            .send()
            .await
            .context("upload to S3")?;
        if !r.status().is_success() {
            bail!(
                "put: HTTP {}: {}",
                r.status(),
                r.text().await.unwrap_or_default()
            );
        }
        let (digest, sent) = {
            let mut g = hasher.lock().expect("lock");
            (std::mem::take(&mut g.0).finalize(), g.1)
        };
        if sent != size {
            // S3 checks Content-Length itself, so this means a broken stream.
            let _ = self.delete(&key).await;
            bail!("upload ended after {sent} of {size} bytes");
        }
        Ok(FileRecord {
            id,
            name: name.to_owned(),
            size,
            sha256: hex::encode(digest),
            created_at: (id / 1_000_000) as u64,
            blocks: 1,
        })
    }

    /// A short-lived URL that downloads the object under its file name.
    pub async fn download_url(&self, key: &str, name: &str) -> anyhow::Result<String> {
        let creds = self.credentials().await?;
        let mut action = self.bucket.get_object(Some(&creds), key);
        let disposition = format!("attachment; filename=\"{name}\"");
        action
            .query_mut()
            .insert("response-content-disposition", disposition);
        Ok(action.sign(SIGN_FOR).to_string())
    }

    pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
        let creds = self.credentials().await?;
        let url = self.bucket.delete_object(Some(&creds), key).sign(SIGN_FOR);
        let r = self.http.delete(url).send().await?;
        if !r.status().is_success() {
            bail!("delete: HTTP {}", r.status());
        }
        Ok(())
    }
}

/// Microseconds since the epoch, so ids sort by upload time and two uploads
/// in the same millisecond still differ.
fn new_id() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    now.as_micros() as i64
}

pub fn key_for(id: i64, name: &str) -> String {
    format!("{PREFIX}{id}/{name}")
}

/// `files/{id}/{name}` back to a record. Other keys are ignored.
pub fn record_from_key(key: &str, size: u64) -> Option<FileRecord> {
    let rest = key.strip_prefix(PREFIX)?;
    let (id, name) = rest.split_once('/')?;
    let id: i64 = id.parse().ok()?;
    if name.is_empty() {
        return None;
    }
    Some(FileRecord {
        id,
        name: name.to_owned(),
        size,
        sha256: String::new(),
        created_at: (id / 1_000_000) as u64,
        blocks: 1,
    })
}

async fn instance_credentials(http: &reqwest::Client, imds: &str) -> anyhow::Result<Credentials> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct RoleCreds {
        access_key_id: String,
        secret_access_key: String,
        token: String,
    }
    let token = http
        .put(format!("{imds}/latest/api/token"))
        .header("x-aws-ec2-metadata-token-ttl-seconds", "21600")
        .send()
        .await
        .context("IMDSv2 token")?
        .error_for_status()?
        .text()
        .await?;
    let role = http
        .get(format!("{imds}/latest/meta-data/iam/security-credentials/"))
        .header("x-aws-ec2-metadata-token", &token)
        .send()
        .await?
        .error_for_status()
        .context("no instance role")?
        .text()
        .await?;
    let role = role.lines().next().unwrap_or("").trim().to_owned();
    let c: RoleCreds = http
        .get(format!(
            "{imds}/latest/meta-data/iam/security-credentials/{role}"
        ))
        .header("x-aws-ec2-metadata-token", &token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(Credentials::new_with_token(
        c.access_key_id,
        c.secret_access_key,
        c.token,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        let key = key_for(1_794_760_000_123_456, "report v2.pdf");
        assert_eq!(key, "files/1794760000123456/report v2.pdf");
        let f = record_from_key(&key, 10).unwrap();
        assert_eq!(
            (f.id, f.name.as_str(), f.size),
            (1_794_760_000_123_456, "report v2.pdf", 10)
        );
        assert_eq!(f.created_at, 1_794_760_000);
    }

    #[test]
    fn foreign_keys_are_ignored() {
        assert!(record_from_key("other/1/x", 1).is_none());
        assert!(record_from_key("files/notanid/x", 1).is_none());
        assert!(record_from_key("files/12/", 1).is_none());
    }
}
