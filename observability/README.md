# Observability

Prometheus and Grafana for the Satchel cluster (#21). Every service exports its metrics on `GET /metrics`, from `satchel_common::metrics`.

| File | What it does |
|---|---|
| `prometheus.yml` | Scrape targets, with a `service` label and a `zone` label for storage nodes |
| `grafana/provisioning/` | Prometheus datasource and the dashboard provider, loaded at start |
| `grafana/dashboards/satchel-overview.json` | The **Satchel overview** dashboard |

## Metrics

| Metric | Type | Labels |
|---|---|---|
| `satchel_http_requests_total` | Counter | `method`, `route`, `status` |
| `satchel_http_request_duration_seconds` | Histogram, 0.5 ms to about 16 s | `method`, `route` |
| `satchel_node_stored_bytes` | Gauge, storage nodes only | none |
| `process_cpu_seconds_total`, `process_resident_memory_bytes` and other `process_*` | From the Linux process collector | none |

`route` is the route pattern, for example `/blocks/{hash}`, never the raw path, so a block hash never becomes a label. Requests that match no route are counted as `unmatched`.

The dashboard's latency percentiles come from histogram buckets, so they are approximate. Experiment results use the exact percentiles the load generator (#22) records.

## Changing the dashboard

Edit it in Grafana, then use **Export > Export as JSON** and replace `grafana/dashboards/satchel-overview.json`. The provisioned copy is read-only in the UI.

## On AWS

The live demo (#63) can run the same stack on the control instance. Start Prometheus and Grafana there with these files, and point the targets in `prometheus.yml` at the private IPs of the instances that #20 creates. The security group only needs to allow the control instance to reach each service's port.
