# Changelog

## Unreleased

### Files service
- An upload that doesn't finish (a full disk, a failed block write, the browser closing) now releases the blocks it wrote, unless a file or another upload in progress uses them. Before, they stayed in the Block Index with no file pointing at them, so even `satchel compact` couldn't reclaim them

## v2.0.0, 2026-11-17

Compaction and coded reads: volumes stay replicated until `satchel compact` reclaims deleted space and codes them as RS(6,3), and every read works from either layout.

### Storage
- **`satchel compact`**: closes written volumes, rewrites live blocks around deleted ones, encodes RS(6,3), checks every live block in the new layout and only then deletes the old one. It refuses to run without healthy nodes in all three zones, and only one runs at a time (DDL-008, 013)
- **Coded reads**: blocks on RS(6,3) volumes are read from the data fragments that hold them, and rebuilt from any six fragments when some are missing (DDL-007)
- **Three fragment slots per zone** also on a one-node-per-zone install, so the local install codes too and survives losing one node (DDL-007)
- Coding no longer starts on its own when a volume closes; the `encode` Terraform variable and `SATCHEL_ENCODE` are gone

### Website
- **My files** groups files into compacted and not yet compacted, shows each file's coded and replicated blocks, the disk used in each zone by layout, and the space the next compaction will reclaim
- **Calculator redesign**: clouds against a Magic Pocket-style owned system, basic and advanced inputs with sourced help, scale comparisons, result slides and a technical mode. Satchel on EC2 is no longer shown
- **How it works** covers blocks and extents, the read path, coding at compaction and deletes

## v1.1.0, 2026-11-16

The Satchel website: store files on AWS S3 (rent) or on Satchel running on your own computer (own), and see what each does with them and what it costs.

### Website
- **React site** (`web/app`, React, Vite, TypeScript): home, store files, my files, the calculator and a "How it works" section built from the reports with source IDs. Light and dark mode, phone width
- **Store on your own Satchel**: the page finds a local install on 127.0.0.1:47800, or shows the one-line install command. Uploads show the file's blocks, deduplicated blocks and the copies landing in each zone
- **Live costs**: the monthly cost of exactly the stored files on S3 and on Satchel on-prem, and a slider that scales the data up to the break-even point, from the same Rust cost engine compiled to WASM
- **Pricing calculator** (`satchel-calc`, `web/calculator`): the cost engine in the browser, matching the CLI exactly

### Files service
- **S3 backend** (`SATCHEL_BACKEND=s3`): each file is one object in S3, with 100 MiB per file and 2 GiB in total
- **Admin login** for S3: Argon2id hashes in SSM (`scripts/set-admin.sh`), HttpOnly, Secure, SameSite=Strict session cookies, at most 2 password checks at once and a growing wait after repeated wrong passwords

### AWS
- **Website stack** (`infra/web`): private S3 buckets, CloudFront for the site and the API, the backend in a private subnet behind a CloudFront VPC origin, and a $50 monthly budget that stops the backend

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
