# Scaling storage nodes

The Master reads `SATCHEL_NODES` on each verifier start. Add a node to that
list and restart the verifier to make it eligible for new volume placement.
Existing volumes stay where their Replication Table rows say they are.

Run the supported local shapes with either command:

```bash
docker compose -f docker-compose.yml -f docker-compose.12.yml up --build
docker compose -f docker-compose.yml -f docker-compose.18.yml up --build
```

The 12- and 18-node overrides also replace Prometheus targets. New volumes use
the full node list after the verifier restarts.

Node draining is not implemented yet. The repair worker reacts to an
unreachable node; it does not provide a safe procedure to move data off a
healthy node or remove its old extents. Do not remove a storage node while it
holds data. A future drain procedure must copy each extent, commit the updated
Replication Table row, delete the old extent, then confirm `GET /extents` is
empty and a frontend read still returns the expected block.
