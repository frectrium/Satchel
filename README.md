<div align="center">

# 🎒 Satchel

**A pocket-sized clone of Dropbox's Magic Pocket.**

Satchel rebuilds the core of the storage system Dropbox wrote to move off AWS S3, at small scale. It justifies every design choice against Dropbox's public sources and asks one question: *at what scale does owning storage beat renting it?*

[![CI](https://github.com/frectrium/Satchel/actions/workflows/ci.yml/badge.svg)](https://github.com/frectrium/Satchel/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.91%2B-orange.svg?logo=rust)](https://www.rust-lang.org)
[![Board](https://img.shields.io/badge/project-board-8957e5.svg?logo=github)](https://github.com/users/frectrium/projects/1)

[Project brief](docs/SATCHEL_PROJECT.md) · [Issues](https://github.com/frectrium/Satchel/issues) · [Milestones](https://github.com/frectrium/Satchel/milestones) · [Contributing](CONTRIBUTING.md)

</div>

---

## Why this project

In 2016 Dropbox announced it had moved most user file data off **AWS S3** and onto **Magic Pocket**, an exabyte-scale block storage system running in its own data centers. It later reported large infrastructure savings. Moving workloads off the public cloud like this is called **cloud repatriation**.

Satchel is a cloud computing case study in two halves:

1. **📚 Case study.** Use primary sources to work out why Dropbox left S3, how Magic Pocket is built, what it cost, and how it turned out.
2. **🛠️ Exercise.** Build a working, scaled-down Magic Pocket, deploy it across three AWS availability zones, measure it, and model the case's workload cost on **AWS**, **Azure** and an **on-premises** estimate.

## What Satchel builds

Each component of Satchel mirrors a component of Magic Pocket:

| Satchel | Role | Magic Pocket counterpart |
|---|---|---|
| `satchel-api` | Accepts put, get and delete requests for blocks | Frontends |
| `satchel-common::BlockHash` | Immutable blocks keyed by content hash, so identical data is stored once | Block storage model |
| `satchel-index` | Maps each block hash to the storage nodes holding it | Block Index |
| `satchel-node` | Stores block data on disk | OSDs |
| Replication | Keeps copies of each block across zones | Cross-zone replication |
| Erasure coding | Data and parity fragments across nodes, so the cluster survives node loss with less overhead than full copies | Erasure-coded volumes |
| `satchel-verifier` | Background integrity checks of stored blocks | Pocket Watch |

The deployment pieces cover what Dropbox ran on its own hardware: multi-AZ placement, Docker, Terraform, horizontal scaling, and Prometheus with Grafana for observability.

```mermaid
flowchart LR
    C([Client / load generator]) -->|put / get / delete| API[satchel-api<br/><i>Frontend</i>]
    API <-->|hash → nodes| IDX[(satchel-index<br/><i>Block Index</i>)]
    API --> N1
    API --> N2
    API --> N3
    subgraph Z1 [Zone 1 · ap-south-1a]
        N1[satchel-node<br/><i>OSD</i>]
    end
    subgraph Z2 [Zone 2 · ap-south-1b]
        N2[satchel-node<br/><i>OSD</i>]
    end
    subgraph Z3 [Zone 3 · ap-south-1c]
        N3[satchel-node<br/><i>OSD</i>]
    end
    V[satchel-verifier<br/><i>Pocket Watch</i>] -.->|scan + check| N1 & N2 & N3
    V -.-> IDX
```

> **Out of scope:** exabyte scale, production-grade durability guarantees, custom hardware (we model it on paper instead), and Dropbox's sync engine and metadata systems.

## Getting started

### Prerequisites

| Tool | Why | Install |
|---|---|---|
| Rust (stable, ≥ 1.91) | Build Satchel | [rustup.rs](https://rustup.rs) |
| Docker + Compose v2 | Run the local multi-zone cluster | [Docker Desktop](https://www.docker.com/products/docker-desktop/) |
| `gh` (optional) | Issues and PRs from the terminal | `brew install gh` |

### Clone and build

```bash
git clone https://github.com/frectrium/Satchel.git
cd Satchel
cargo build --workspace
cargo test --workspace
```

### Run the local cluster

This starts one frontend, the Block Index, the verifier, and one storage node in each of three simulated zones:

```bash
docker compose up --build
```

```bash
curl localhost:8080/health
```

| Service | Host port | Zone |
|---|---|---|
| `api` | 8080 | none |
| `index` | 8081 | none |
| `verifier` | 8085 | none |
| `node-z1` / `node-z2` / `node-z3` | 8091 / 8092 / 8093 | z1 / z2 / z3 |

You can also run a single service without Docker:

```bash
SATCHEL_PORT=8090 SATCHEL_ZONE=z1 cargo run -p satchel-node
```

> **Status:** the services are skeletons that serve `GET /health`. The storage path gets built in Week 3. See the [roadmap](#roadmap).

## Repository layout

```
Satchel/
├── crates/
│   ├── satchel-common/     # BlockHash, service config, /health, shutdown
│   ├── satchel-api/        # Frontend
│   ├── satchel-index/      # Block Index
│   ├── satchel-node/       # Storage node (OSD)
│   └── satchel-verifier/   # Background verifier (Pocket Watch)
├── docs/
│   ├── SATCHEL_PROJECT.md  # Project brief
│   └── sources/            # Source Log: every citation, with a stable ID
├── .github/                # CI, issue forms, PR template, CODEOWNERS
├── Dockerfile              # One image, all binaries
└── docker-compose.yml      # Local 3-zone dev cluster
```

## Roadmap

| Week | Focus | Deliverable |
|---|---|---|
| **1** | Find primary sources. Extract architecture, cost and outcome data | One-page summary of sources and key points |
| **2** | Case analysis, architecture diagram, exercise plan (incl. erasure coding parameters), Team Roles, Design Decision Log | 10-page report |
| **3** | Build Satchel's core, deploy to AWS, measure latency, scaling and cost | Report add-ons with results |
| **4** | Report preparation. Fold in the Design Decision Log | Final report |
| **5** | Project viva | Viva presentation |

Progress is tracked in the [milestones](https://github.com/frectrium/Satchel/milestones) and on the [project board](https://github.com/users/frectrium/projects/1).

## Team

| | Member |
|---|---|
| <img src="https://github.com/frectrium.png" width="40"> | [@frectrium](https://github.com/frectrium) |
| <img src="https://github.com/VyasGuru.png" width="40"> | [@VyasGuru](https://github.com/VyasGuru) |
| <img src="https://github.com/jasmehta08.png" width="40"> | [@jasmehta08](https://github.com/jasmehta08) |

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) for branch naming, commit style, PR rules and how we handle sources.

## License

[MIT](LICENSE).

<sub>Satchel is an independent educational project. It is not affiliated with or endorsed by Dropbox, Inc. "Dropbox" and "Magic Pocket" are trademarks of their respective owners.</sub>
