# Changelog

## v1.0.0, 2026-11-16

The Satchel storage system: a pocket-sized Magic Pocket, with every design decision traced to a source in the [Design Decision Log](docs/design/decision-log.md).

### Storage
- **Frontend** (`satchel-api`): put, get and delete for blocks of up to 4 MiB keyed by SHA-256, dedup on put, writes to all 3 replicas of an open volume and retry on another volume (DDL-001, 002, 006)
- **Block Index and Replication Table** (`satchel-index`): one SQLite store, volume generations with compare-and-set, no new blocks on closed volumes (DDL-003, 019)
- **Storage nodes** (`satchel-node`): append-only extent files with the hash and length inline, fsync before acknowledging, fragments for coded volumes (DDL-011, 018)
- **Master** (in `satchel-verifier`): opens volumes with one replica per zone, closes them by size or age, encodes closed volumes as RS(6,3) with three fragments per zone (DDL-004, 006, 007, 008, 009)
- **Erasure coding**: the RS(6,3) codec in `satchel-common` (DDL-007)
- **Verifier**: Disk Scrubber, Metadata Scanner and repair after a node is down past a threshold (DDL-010, 012)

### Files and local install
- **Files service** (`satchel-files`): files as ordered block lists in a SQLite manifest, with reference counts so deleting a file drops only blocks no other file uses (DDL-020)
- **One-command install** for macOS and Linux (`scripts/install.sh`) and the `satchel` launcher, with every service on 127.0.0.1
- Release builds for macOS (arm64, x86_64) and Linux (x86_64, arm64), attached to each GitHub release

### Operations
- Prometheus metrics on every service and a Grafana dashboard (DDL-017)
- Docker image for every service, published for arm64 to GitHub Container Registry
- Terraform for the 3-AZ demo cluster in ap-south-1 (DDL-005, 015, 016)
- Load generator for Satchel and S3, and integration tests on the Compose cluster

### Cost model
- Sizing model, the `satchel-cost` engine and CLI, and the AWS, Azure and on-prem estimates with the break-even data volume

### Known limits
- Reads from erasure-coded volumes are not in v1.0 (#70). The local install and the demo keep volumes replicated.
- The file manifest is one SQLite file per install, not a replicated metadata service (DDL-020).
