//! Integration test harness: which services a Satchel cluster has, where they
//! are and how long to wait for them. The tests are in `tests/`.
//!
//! The service list comes from `docker-compose.yml`, so the suite always covers
//! every service declared there, however many nodes the cluster has.
//!
//! Magic Pocket counterpart: release testing. Dropbox runs integration tests
//! with failure injection before a release moves on [B3].

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::Value;

/// How long to wait for every service to answer `/health`, unless
/// `SATCHEL_IT_READY_TIMEOUT_SECS` says otherwise.
pub const DEFAULT_READY_TIMEOUT: Duration = Duration::from_secs(60);

/// One service the suite talks to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    /// Compose service name, e.g. `node-z1`.
    pub name: String,
    /// Name the service reports in `/health`, e.g. `satchel-node`.
    pub reports_as: String,
    /// Zone the service must report, for storage nodes.
    pub zone: Option<String>,
    /// Base URL without a trailing slash, e.g. `http://localhost:8091`.
    pub base_url: String,
}

/// Every Satchel service of the cluster, with its base URL resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cluster {
    pub services: Vec<Service>,
    pub ready_timeout: Duration,
}

impl Cluster {
    /// Reads the services from `docker-compose.yml` and the `SATCHEL_IT_*`
    /// environment variables.
    pub fn from_env() -> Result<Self, String> {
        let compose_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docker-compose.yml");
        let output = Command::new("docker")
            .args(["compose", "-f"])
            .arg(&compose_file)
            .args(["config", "--format", "json"])
            .output()
            .map_err(|e| format!("could not run `docker compose config`: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "`docker compose config` failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let config = String::from_utf8_lossy(&output.stdout);
        Self::from_compose(&config, |key| std::env::var(key).ok())
    }

    /// Builds the cluster from `docker compose config --format json` output.
    /// Each service's URL defaults to its published host port, and
    /// `SATCHEL_IT_<SERVICE>_URL` (from `lookup`) overrides it, for example
    /// `SATCHEL_IT_NODE_Z1_URL` for `node-z1`.
    pub fn from_compose(
        config: &str,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, String> {
        let config: Value = serde_json::from_str(config)
            .map_err(|e| format!("`docker compose config` output is not JSON: {e}"))?;
        let declared = config["services"]
            .as_object()
            .ok_or("`docker compose config` output has no services")?;

        let mut services = Vec::new();
        for (name, service) in declared {
            // Anything else in the file (Prometheus, say) isn't ours to check.
            let Some(reports_as) = satchel_binary(service) else {
                continue;
            };
            let var = url_var(name);
            let base_url = match lookup(&var) {
                Some(url) => parse_base_url(&var, &url)?,
                None => format!("http://localhost:{}", published_port(name, service)?),
            };
            services.push(Service {
                name: name.clone(),
                reports_as: reports_as.to_owned(),
                zone: service["environment"]["SATCHEL_ZONE"]
                    .as_str()
                    .map(str::to_owned),
                base_url,
            });
        }
        if services.is_empty() {
            return Err("docker-compose.yml declares no Satchel services".to_owned());
        }

        let ready_timeout = match lookup("SATCHEL_IT_READY_TIMEOUT_SECS") {
            Some(secs) => parse_timeout(&secs)?,
            None => DEFAULT_READY_TIMEOUT,
        };

        Ok(Self {
            services,
            ready_timeout,
        })
    }
}

/// The Satchel binary a Compose service runs, if it runs one.
fn satchel_binary(service: &Value) -> Option<&str> {
    service["command"][0]
        .as_str()
        .filter(|command| command.starts_with("satchel-"))
}

/// `node-z1` becomes `SATCHEL_IT_NODE_Z1_URL`.
fn url_var(service: &str) -> String {
    format!(
        "SATCHEL_IT_{}_URL",
        service.to_uppercase().replace('-', "_")
    )
}

/// The first published host port. Compose prints it as a string.
fn published_port(name: &str, service: &Value) -> Result<u16, String> {
    service["ports"][0]["published"]
        .as_str()
        .and_then(|port| port.parse().ok())
        .ok_or_else(|| format!("{name} publishes no host port in docker-compose.yml"))
}

fn parse_base_url(var: &str, raw: &str) -> Result<String, String> {
    let url = raw.trim().trim_end_matches('/');
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(url.to_owned())
    } else {
        Err(format!(
            "{var} must be an http:// or https:// URL, got {raw:?}"
        ))
    }
}

fn parse_timeout(raw: &str) -> Result<Duration, String> {
    match raw.trim().parse::<u64>() {
        Ok(secs) if secs > 0 => Ok(Duration::from_secs(secs)),
        _ => Err(format!(
            "SATCHEL_IT_READY_TIMEOUT_SECS must be a whole number of seconds above 0, got {raw:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Same shape as `docker compose config --format json`.
    const CONFIG: &str = r#"{
        "services": {
            "api": {
                "command": ["satchel-api"],
                "environment": {"SATCHEL_PORT": "8080"},
                "ports": [{"target": 8080, "published": "8080"}]
            },
            "node-z1-2": {
                "command": ["satchel-node"],
                "environment": {"SATCHEL_PORT": "8090", "SATCHEL_ZONE": "z1"},
                "ports": [{"target": 8090, "published": "8092"}]
            },
            "prometheus": {
                "command": ["--config.file=/etc/prometheus.yml"],
                "ports": [{"target": 9090, "published": "9090"}]
            }
        }
    }"#;

    fn cluster(config: &str, vars: &[(&str, &str)]) -> Result<Cluster, String> {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Cluster::from_compose(config, |key| vars.get(key).cloned())
    }

    #[test]
    fn takes_every_satchel_service_from_compose() {
        let c = cluster(CONFIG, &[]).unwrap();
        assert_eq!(
            c.services,
            [
                Service {
                    name: "api".to_owned(),
                    reports_as: "satchel-api".to_owned(),
                    zone: None,
                    base_url: "http://localhost:8080".to_owned(),
                },
                Service {
                    name: "node-z1-2".to_owned(),
                    reports_as: "satchel-node".to_owned(),
                    zone: Some("z1".to_owned()),
                    base_url: "http://localhost:8092".to_owned(),
                },
            ]
        );
        assert_eq!(c.ready_timeout, DEFAULT_READY_TIMEOUT);
    }

    #[test]
    fn env_overrides_one_url_and_strips_trailing_slash() {
        let c = cluster(
            CONFIG,
            &[("SATCHEL_IT_NODE_Z1_2_URL", " http://10.0.2.15:8090/ ")],
        )
        .unwrap();
        assert_eq!(c.services[0].base_url, "http://localhost:8080");
        assert_eq!(c.services[1].base_url, "http://10.0.2.15:8090");
    }

    #[test]
    fn rejects_url_without_scheme() {
        let err = cluster(CONFIG, &[("SATCHEL_IT_API_URL", "localhost:8080")]).unwrap_err();
        assert!(err.contains("SATCHEL_IT_API_URL"), "{err}");
    }

    #[test]
    fn rejects_service_without_published_port() {
        let config = r#"{"services": {"api": {"command": ["satchel-api"]}}}"#;
        let err = cluster(config, &[]).unwrap_err();
        assert!(err.contains("api publishes no host port"), "{err}");
    }

    #[test]
    fn rejects_config_without_satchel_services() {
        for config in [r#"{"services": {}}"#, r#"{}"#, "not json"] {
            assert!(cluster(config, &[]).is_err(), "{config}");
        }
    }

    #[test]
    fn reads_timeout() {
        let c = cluster(CONFIG, &[("SATCHEL_IT_READY_TIMEOUT_SECS", "120")]).unwrap();
        assert_eq!(c.ready_timeout, Duration::from_secs(120));
    }

    #[test]
    fn rejects_zero_or_non_numeric_timeout() {
        for bad in ["0", "-5", "ten", ""] {
            let err = cluster(CONFIG, &[("SATCHEL_IT_READY_TIMEOUT_SECS", bad)]).unwrap_err();
            assert!(
                err.contains("SATCHEL_IT_READY_TIMEOUT_SECS"),
                "{bad}: {err}"
            );
        }
    }
}
