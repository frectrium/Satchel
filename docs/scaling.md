# Scaling storage nodes

The Master reads `SATCHEL_NODES` on each verifier start. Add a node to that
list and restart the verifier to make it eligible for new volume placement.
Existing volumes stay where their Replication Table rows say they are; the
repair worker can copy their extents before a drained node is removed.

Run the supported local shapes with either command:

```bash
docker compose -f docker-compose.yml -f docker-compose.12.yml up --build
docker compose -f docker-compose.yml -f docker-compose.18.yml up --build
```

The 12- and 18-node overrides also replace Prometheus targets. New volumes use
the full node list after the verifier restarts. A drain is complete when the
removed node's `GET /extents` response is empty and a frontend read still
returns the expected block.
