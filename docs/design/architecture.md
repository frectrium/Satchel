# Architecture: Magic Pocket and Satchel

Issue #11. Two matching diagrams: Magic Pocket as Dropbox described it, and Satchel as we're building it, with the put and get paths numbered. Objective 2 in the [project brief](../SATCHEL_PROJECT.md).

- The Magic Pocket half is drawn from the architecture notes ([`architecture.md`](../research/architecture.md), #8). Source IDs refer to the [Source Log](../sources/README.md).
- The Satchel half follows the exercise plan ([`exercise-plan.md`](exercise-plan.md), #14). Each simplification points to its entry in the [Design Decision Log](decision-log.md) (#17).
- The diagrams are Mermaid, so GitHub renders them and they diff in PRs.

## 1. Magic Pocket

### 1.1 Components

The layout is the 2016 design in [B2], with the erasure codes named in 2024 [T9]. Zone B has the same layout as zone A.

```mermaid
flowchart TB
    S([Dropbox services]) -->|put / get| FE[Frontends]
    FE <-->|"hash → cell, bucket"| BI[("Block Index<br/>sharded MySQL")]
    FE -.->|"poll free space, open volumes"| CELL

    subgraph ZA ["Zone A"]
        direction TB
        subgraph CELL ["Cell (about 50 PB raw in 2016)"]
            direction TB
            RT[("Replication Table<br/>MySQL")]
            M["Master<br/>off the data path"]
            subgraph OPEN ["Open volume: 4 replicas"]
                O1[OSD] & O2[OSD] & O3[OSD] & O4[OSD]
            end
            subgraph CLOSED ["Closed volume: erasure coded"]
                F1[OSD] & F2[OSD] & F3["OSD ..."]
            end
            VM["Volume Managers<br/>run on OSD machines"]
        end
    end

    FE <-->|"bucket → volume → OSDs"| RT
    FE -->|write to every replica| OPEN
    FE -->|read from one OSD| CLOSED
    M -.->|"repair, encode, GC jobs"| VM
    VM -.->|"encode closed volumes"| CLOSED
    ZA ==>|"async cross-zone copy, under 1 s"| ZB["Zone B<br/>same layout"]
    PW["Pocket Watch<br/>6 verifiers"] -.->|"scan and check"| BI & CELL & ZB
```

### 1.2 Put path

Steps from [B2], except step 8 [B2][B5].

```mermaid
sequenceDiagram
    autonumber
    participant S as Dropbox service
    participant FE as Frontend
    participant BI as Block Index
    participant RT as Replication Table
    participant O as OSDs of an open volume (zone A)
    participant X as Cross-zone daemon
    participant ZB as Zone B
    S->>FE: put(hash, block)
    FE->>BI: does the hash exist?
    BI-->>FE: no (if yes, stop here: dedup)
    FE->>RT: OSDs for a chosen open volume
    FE->>O: write block to every OSD
    O-->>FE: ack after fsync (on failure, retry another volume)
    FE->>BI: add hash → cell, bucket
    X->>ZB: copy the block to the other zone (async, under 1 s)
    Note over O: Later, when the volume closes, Volume Managers erasure code it in the background
```

If the Block Index fails during a put, the Frontend forwards the request to the other zone [B2].

### 1.3 Get path

Steps from [B2].

```mermaid
sequenceDiagram
    autonumber
    participant S as Dropbox service
    participant FE as Frontend
    participant BI as Block Index
    participant RT as Replication Table
    participant O as OSD holding the block
    participant VM as Volume Manager
    S->>FE: get(hash)
    FE->>BI: look up the cell and bucket
    FE->>RT: look up the volume and its OSDs
    FE->>O: read the block (whole on one OSD, even when coded)
    O-->>FE: block
    alt that OSD is down
        FE->>VM: reconstruct from the other fragments
        VM-->>FE: block
    end
    FE-->>S: block
```

### 1.4 What each component does

| Component | Role | Source |
|---|---|---|
| Block | Immutable chunk of a file, up to 4 MB, keyed by SHA-256. Arrives compressed and encrypted | [B2] |
| Frontend | Gateway for puts and gets. Picks the volume for each new block to spread load and limit cross-cluster traffic | [B2] |
| Block Index | Maps each hash to a cell and bucket. A sharded MySQL cluster behind an RPC layer | [B2] |
| Zone | Geographic location holding a full copy of each block. At least two per block, thousands of miles apart | [B2], [B5] |
| Cell | Self-contained storage cluster, about 50 PB raw in 2016 and 100 to 120 PB in 2024. Capacity grows by adding cells | [B2], [T9] |
| Bucket and volume | Bucket: 1 GB of blocks written around the same time (1 to 4 GB by 2024). Volume: buckets stored on a set of OSDs, open for writes or closed and immutable | [B2], [T9] |
| Replication Table | Maps buckets to volumes and volumes to OSDs, with the volume's type and generation. MySQL, small enough to stay in memory | [B2] |
| Master | One per cell. Triggers repair, creates buckets, runs garbage collection. Soft state, so it stays off the data path | [B2] |
| Volume Manager | Moves and erasure codes volumes. Runs on the OSD machines to use their idle network capacity | [B2] |
| OSD | "Dumb" storage node: one daemon per disk, raw disk with no filesystem, no knowledge of the cell topology | [B2], [B4] |
| Replication | New data goes to open volumes: 4 copies within a zone, in two zones. The Frontend writes to every copy, no quorum | [B2], [T8] |
| Erasure coding | Closed volumes are coded in the background. RS(6,3) or LRC(12,2,2) in 2024. Most reads still hit a single disk | [B2], [T9] |
| Cross-zone replication | A daemon copies every put to the other zone asynchronously, within about a second | [B2] |
| Pocket Watch | Six verifiers, from the Disk Scrubber to the Cross-zone Verifier. Over half of disk and database load | [B3] |

## 2. Satchel

### 2.1 Components

Same shape as section 1.1. One cell spread over three zones, with 3 storage nodes in each. The open volume shows where a new block's 3 replicas go. The closed volume shows where its 9 RS(6,3) fragments go.

```mermaid
flowchart TB
    C([Client or load generator]) -->|"PUT / GET / DELETE /blocks/{hash}"| API["satchel-api<br/>Frontend"]
    API <-->|"hash → volume, offset"| IDX[("satchel-index<br/>SQLite: Block Index<br/>+ Replication Table")]

    subgraph CELL ["One cell, ap-south-1"]
        direction LR
        subgraph Z1 ["Zone z1 (1a)"]
            direction TB
            N11[node-z1-1] ~~~ N12[node-z1-2] ~~~ N13[node-z1-3]
        end
        subgraph Z2 ["Zone z2 (1b)"]
            direction TB
            N21[node-z2-1] ~~~ N22[node-z2-2] ~~~ N23[node-z2-3]
        end
        subgraph Z3 ["Zone z3 (1c)"]
            direction TB
            N31[node-z3-1] ~~~ N32[node-z3-2] ~~~ N33[node-z3-3]
        end
    end

    API -->|"open volume: write all 3 replicas, one per zone"| N11 & N21 & N31
    API -->|"closed volume: read one data fragment"| N12

    VER["satchel-verifier<br/>Master + Pocket Watch"] -.->|"open and close volumes,<br/>schedule encode and repair"| IDX
    VER -.->|"scrub and metadata scan"| CELL
    PROM["Prometheus + Grafana"] -.->|scrape /metrics| API & IDX & VER & CELL
```

A closed volume's 9 fragments use all 9 nodes: 3 per zone, so losing a zone loses 3 fragments, which RS(6,3) tolerates (DDL-007). With 12 or 18 nodes, each volume uses 3 nodes from each zone.

### 2.2 Put path

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant API as satchel-api
    participant IDX as satchel-index
    participant N as 3 nodes, one per zone
    C->>API: PUT /blocks/{hash}
    API->>API: check SHA-256(body) = hash, size 1 B to 4 MiB
    API->>IDX: does the hash exist?
    IDX-->>API: no (if yes, return success: dedup)
    API->>IDX: nodes of one of my open volumes
    API->>N: write block at the next offset, to all 3
    N-->>API: ack after fsync (on failure, retry another open volume)
    API->>IDX: add hash → volume, offset, length
    API-->>C: 201 Created
    Note over N: When the volume reaches 64 MiB or 10 minutes, the Master closes it and a node encodes it into 9 RS(6,3) fragments
```

### 2.3 Get path

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant API as satchel-api
    participant IDX as satchel-index
    participant N as Node holding the block
    participant R as Other fragment holders
    C->>API: GET /blocks/{hash}
    API->>IDX: look up volume, offset, length
    API->>IDX: look up the volume's type and nodes
    API->>N: read the range (a replica, or the data fragment that holds it)
    N-->>API: bytes
    alt node is down (coded volume)
        API->>R: read the same range from any 6 fragments
        R-->>API: fragments
        API->>API: decode RS(6,3)
    end
    API->>API: check SHA-256(bytes) = hash
    API-->>C: 200 with the block
```

### 2.4 What Satchel keeps and what it simplifies

| Magic Pocket | Satchel | Kept | Simplified | DDL |
|---|---|---|---|---|
| Blocks up to 4 MB, SHA-256 | Blocks up to 4 MiB, SHA-256 (`BlockHash`) | Immutable, content-addressed, dedup on put | No compression or encryption | DDL-001, 002 |
| Frontends | `satchel-api` | Gateway for put and get, picks the volume, writes all replicas | One instance, HTTP | DDL-002 |
| Block Index and Replication Table (MySQL) | `satchel-index` (SQLite) | Proven relational store behind a service, two tables | One unsharded database, offsets stored in the index | DDL-003 |
| Buckets of 1 to 4 GB, open and closed volumes | Volumes of 64 MiB, open and closed | Open volumes take writes, closed ones are immutable | Smaller unit, one bucket per volume | DDL-004 |
| Two or more zones far apart, many cells | Three AZs, one cell | Every block spans zones | AZs are kilometres apart, one cell | DDL-005 |
| 4 replicas per zone, async cross-zone copy | 3 replicas, one per zone, all synchronous | Write to every replica, fsync, retry, no quorum | 3 copies, no async daemon | DDL-006 |
| RS(6,3) or LRC(12,2,2) | RS(6,3), 3 fragments per zone | Same code, reads usually hit one node | No LRC, no cold tier | DDL-007, 008 |
| Master, Volume Managers | Master module in the verifier process, nodes act as Volume Managers | Soft-state coordinator off the data path, encoding on storage nodes | Shares a process with the verifier | DDL-009 |
| Repair after 15 minutes | Configurable threshold, 30 s in tests | Close volumes, rebuild, commit by generation | No Reconcile, rebuild on one node | DDL-010 |
| OSDs on raw SMR disks | `satchel-node` with extent files on EBS | Dumb nodes, fsync, inline metadata | Filesystem instead of raw disks | DDL-011 |
| Pocket Watch, six verifiers | `satchel-verifier`, two checks | Disk Scrubber and Metadata Scanner | Four verifiers left out | DDL-012 |
| Garbage collection and compaction | Index entry removed on delete | Deleted blocks can't be read | Space isn't reclaimed | DDL-013 |
| Dropbox's own data centers | EC2 Graviton instances, Terraform, Prometheus and Grafana | | Cloud VMs stand in for storage servers | DDL-015 |
