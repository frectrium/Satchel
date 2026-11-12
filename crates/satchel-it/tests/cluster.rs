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

/// Sends `body` as JSON and returns the status and JSON reply.
fn send_json(agent: &ureq::Agent, method: &str, url: &str, body: &Value) -> (u16, Value) {
    let request = match method {
        "POST" => agent.post(url),
        "PUT" => agent.put(url),
        _ => panic!("unsupported method {method}"),
    };
    let mut response = request
        .send_json(body)
        .unwrap_or_else(|e| panic!("{method} {url}: {e}"));
    let status = response.status().as_u16();
    let reply = response.body_mut().read_json().unwrap_or(Value::Null);
    (status, reply)
}

#[test]
#[ignore = "needs the Compose cluster: run scripts/integration-test.sh"]
fn ts_idx_4_entries_survive_an_index_restart() {
    let cluster = cluster();
    let index = cluster
        .services
        .iter()
        .find(|s| s.name == "index")
        .expect("docker-compose.yml has an index service");
    let agent = agent();
    wait_for_health(&agent, index, Instant::now() + cluster.ready_timeout).unwrap();

    let (status, volume) = send_json(
        &agent,
        "POST",
        &format!("{}/volumes", index.base_url),
        &serde_json::json!({"nodes": ["ts-idx-4-a", "ts-idx-4-b", "ts-idx-4-c"]}),
    );
    assert_eq!(status, 201, "POST /volumes returned {status}: {volume}");
    // SHA-256 of "TS-IDX-4". The index stores entries without the bytes, so
    // any valid hash works, and a rerun on the same cluster reuses it.
    let hash = "c5354852e3c28a6210eec24b103cd7453d5258dd33151e8e7fa75ba1b7caf420";
    let entry = serde_json::json!({"volume": volume["id"], "offset": 0, "length": 8});
    // 201 with this entry, or 200 with the one a previous run stored.
    let (status, stored) = send_json(
        &agent,
        "PUT",
        &format!("{}/blocks/{hash}", index.base_url),
        &entry,
    );
    assert!(
        status == 201 || status == 200,
        "PUT /blocks returned {status}"
    );

    let compose = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docker-compose.yml");
    let restarted = std::process::Command::new("docker")
        .args(["compose", "-f"])
        .arg(&compose)
        .args(["restart", "index"])
        .status()
        .expect("docker compose restart runs");
    assert!(restarted.success(), "docker compose restart index failed");
    wait_for_health(&agent, index, Instant::now() + cluster.ready_timeout).unwrap();

    let mut response = agent
        .get(format!("{}/blocks/{hash}", index.base_url))
        .call()
        .unwrap();
    assert_eq!(
        response.status().as_u16(),
        200,
        "TS-IDX-4: the entry is gone after a restart"
    );
    let got: Value = response.body_mut().read_json().unwrap();
    assert_eq!(got, stored, "TS-IDX-4: the entry changed across a restart");
}
