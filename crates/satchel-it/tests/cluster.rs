//! Integration tests against a running cluster. They are `#[ignore]`d so the
//! plain `cargo test --workspace` (no cluster) skips them. Run them with
//! `scripts/integration-test.sh`, or with `cargo test -p satchel-it -- --ignored`
//! against a cluster that is already up.
//!
//! Test names follow the IDs in `docs/design/test-strategy.md`.

use std::time::{Duration, Instant};

use satchel_it::{Cluster, Service};
use serde_json::Value;

/// Time between `/health` polls while waiting for a service.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

fn cluster() -> Cluster {
    Cluster::from_env().unwrap_or_else(|e| panic!("bad integration test config: {e}"))
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .into()
}

/// One `GET /health`. Returns the HTTP status and body, or why the request failed.
fn get_health(agent: &ureq::Agent, service: &Service) -> Result<(u16, String), String> {
    let mut response = agent
        .get(format!("{}/health", service.base_url))
        .call()
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("HTTP {status}, unreadable body: {e}"))?;
    Ok((status, body))
}

/// Polls `GET /health` until it returns 200 or `deadline` passes, then returns
/// the JSON body. The error names the service, its URL and its last answer.
fn wait_for_health(
    agent: &ureq::Agent,
    service: &Service,
    deadline: Instant,
) -> Result<Value, String> {
    let url = format!("{}/health", service.base_url);
    loop {
        let last = match get_health(agent, service) {
            Ok((200, body)) => {
                return serde_json::from_str(&body).map_err(|e| {
                    format!(
                        "{} at {url} returned 200 but not JSON ({e}): {body}",
                        service.name
                    )
                })
            }
            Ok((status, body)) => format!("HTTP {status}: {body}"),
            Err(e) => e,
        };
        if Instant::now() >= deadline {
            return Err(format!(
                "{} at {url} did not return 200 in time, last answer: {last}",
                service.name
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Waits for each service, checks its `/health` body with `check`, and fails
/// once with every problem found.
fn check_all(
    test_id: &str,
    services: &[&Service],
    ready_timeout: Duration,
    check: impl Fn(&Service, &Value) -> Result<(), String>,
) {
    let agent = agent();
    let deadline = Instant::now() + ready_timeout;
    let failures: Vec<String> = services
        .iter()
        .filter_map(|service| {
            wait_for_health(&agent, service, deadline)
                .and_then(|body| check(service, &body))
                .err()
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{test_id} failed for {} of {} services:\n  {}",
        failures.len(),
        services.len(),
        failures.join("\n  ")
    );
}

#[test]
#[ignore = "needs the Compose cluster: run scripts/integration-test.sh"]
fn ts_svc_1_all_services_healthy() {
    let cluster = cluster();
    let services: Vec<&Service> = cluster.services.iter().collect();
    check_all(
        "TS-SVC-1",
        &services,
        cluster.ready_timeout,
        |service, body| {
            let reported = (&body["service"], &body["status"]);
            if reported
                == (
                    &Value::from(service.reports_as.as_str()),
                    &Value::from("ok"),
                )
            {
                Ok(())
            } else {
                Err(format!(
                    "{} at {}/health: expected service {:?} with status \"ok\", got {body}",
                    service.name, service.base_url, service.reports_as
                ))
            }
        },
    );
}

#[test]
#[ignore = "needs the Compose cluster: run scripts/integration-test.sh"]
fn ts_node_4_nodes_report_their_zone() {
    let cluster = cluster();
    let nodes: Vec<&Service> = cluster
        .services
        .iter()
        .filter(|s| s.zone.is_some())
        .collect();
    assert!(
        !nodes.is_empty(),
        "docker-compose.yml declares no storage nodes"
    );
    check_all(
        "TS-NODE-4",
        &nodes,
        cluster.ready_timeout,
        |service, body| {
            if body["zone"] == Value::from(service.zone.as_deref()) {
                Ok(())
            } else {
                Err(format!(
                    "{} at {}/health: expected zone {:?}, got {body}",
                    service.name,
                    service.base_url,
                    service.zone.as_deref().unwrap_or_default()
                ))
            }
        },
    );
}

#[test]
#[ignore = "needs the Compose cluster: run scripts/integration-test.sh"]
fn ts_svc_3_all_services_export_metrics() {
    let cluster = cluster();
    let services: Vec<&Service> = cluster.services.iter().collect();
    let agent = agent();
    check_all(
        "TS-SVC-3",
        &services,
        cluster.ready_timeout,
        |service, _health| {
            let url = format!("{}/metrics", service.base_url);
            let mut response = agent
                .get(&url)
                .call()
                .map_err(|e| format!("{} at {url}: {e}", service.name))?;
            let status = response.status().as_u16();
            let body = response.body_mut().read_to_string().map_err(|e| {
                format!(
                    "{} at {url}: HTTP {status}, unreadable body: {e}",
                    service.name
                )
            })?;
            // The /health polls above are already counted. process_* comes
            // from the Linux process collector.
            let mut expected = vec![
                "satchel_http_requests_total{",
                "satchel_http_request_duration_seconds_bucket{",
                "process_resident_memory_bytes ",
            ];
            if service.zone.is_some() {
                expected.push("satchel_node_stored_bytes ");
            }
            let missing: Vec<&str> = expected
                .into_iter()
                .filter(|metric| !body.contains(metric))
                .collect();
            if status == 200 && missing.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "{} at {url}: HTTP {status}, missing {missing:?}",
                    service.name
                ))
            }
        },
    );
}
