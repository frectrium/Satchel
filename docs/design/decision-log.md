# Design Decision Log

Issue #17. Every Satchel design decision, the Magic Pocket decision it mirrors, the reason Dropbox gave, and what Satchel simplifies. The exercise plan ([`exercise-plan.md`](exercise-plan.md), #14) explains how the decisions fit together. This log is the record. It's filled in through Week 3 and folded into the final report in Week 4 (#27).

Source IDs refer to the [Source Log](../sources/README.md). Background for most entries is in the architecture notes ([`architecture.md`](../research/architecture.md)).

## How to add an entry

1. Take the next free ID (`DDL-NNN`). IDs never change and are never reused. A decision that's replaced keeps its entry and its status becomes **Superseded by DDL-NNN**.
2. Copy the template below and fill in every field. If Magic Pocket has no counterpart (for example a cloud-only choice), write **None** and mark the entry as a **deliberate deviation**. The team agrees on deviations in the PR.
3. Quote or paraphrase Dropbox's reasoning only from a source in the Source Log. Anything we work out ourselves is marked *(our inference)*.
4. Add the entry to the index table.
5. Link the entry from the PR that makes the decision. The PR template has a checkbox for this.

**Status values:** Proposed (in an open PR), Accepted (merged), Superseded, Rejected.

### Template

```markdown
### DDL-NNN: <short title>

| Field | Value |
|---|---|
| Satchel decision | |
| Magic Pocket equivalent | |
| Dropbox's stated reasoning | |
| Source | |
| What Satchel simplifies | |
| Status / date | Proposed, YYYY-MM-DD |
```

## Index

| ID | Decision | Component | Status |
|---|---|---|---|
| [DDL-001](#ddl-001-immutable-content-addressed-blocks-of-up-to-4-mib) | Immutable, content-addressed blocks of up to 4 MiB | Block model | Proposed |
| [DDL-002](#ddl-002-block-api-with-dedup-on-put) | Block API with dedup on put | Frontend | Proposed |
| [DDL-003](#ddl-003-block-index-and-replication-table-in-one-relational-store) | Block Index and Replication Table in one relational store | Block Index | Proposed |
| [DDL-004](#ddl-004-buckets-and-volumes-of-64-mib-open-then-closed) | Buckets and volumes of 64 MiB, open then closed | Storage layout | Proposed |
| [DDL-005](#ddl-005-three-aws-availability-zones-one-cell) | Three AWS availability zones, one cell | Zones and cells | Proposed |
| [DDL-006](#ddl-006-three-synchronous-replicas-one-per-zone-for-new-data) | Three synchronous replicas, one per zone, for new data | Replication | Proposed |
| [DDL-007](#ddl-007-rs63-erasure-coding-with-three-fragments-per-zone) | RS(6,3) erasure coding with three fragments per zone | Erasure coding | Proposed |
| [DDL-008](#ddl-008-closed-volumes-are-erasure-coded-in-the-background) | Closed volumes are erasure coded in the background | Erasure coding | Proposed |
| [DDL-009](#ddl-009-master-duties-run-off-the-data-path) | Master duties run off the data path | Coordination | Proposed |
| [DDL-010](#ddl-010-repair-after-a-configurable-offline-threshold) | Repair after a configurable offline threshold | Repair | Proposed |
| [DDL-011](#ddl-011-storage-nodes-store-extents-as-files-and-fsync-before-acknowledging) | Storage nodes store extents as files and fsync before acknowledging | Storage node | Proposed |
| [DDL-012](#ddl-012-verifier-implements-the-disk-scrubber-and-metadata-scanner) | Verifier implements the Disk Scrubber and Metadata Scanner | Verifier | Proposed |
| [DDL-013](#ddl-013-deletes-remove-the-index-entry-no-compaction) | Deletes remove the index entry, no compaction | Deletes | Proposed |
| [DDL-014](#ddl-014-satchel-is-written-in-rust) | Satchel is written in Rust | All | Proposed |
| [DDL-015](#ddl-015-graviton-ec2-instances-for-measured-runs) | Graviton EC2 instances for measured runs | Deployment | Proposed (deviation) |

## Entries

### DDL-001: Immutable, content-addressed blocks of up to 4 MiB

| Field | Value |
|---|---|
| Satchel decision | A block is an opaque byte string of 1 byte to 4 MiB (4,194,304 bytes). Its key is the SHA-256 of its contents (`BlockHash` in `satchel-common`). A block never changes after it's written. Empty blocks are rejected |
| Magic Pocket equivalent | Immutable blocks "up to 4MB in size", keyed for most use cases by their SHA-256 hash. Mutability lives higher up, in FileJournal [B2] |
| Dropbox's stated reasoning | "Immutability makes our lives a lot easier": it moves the logic for mutation higher up the stack [B2]. Magic Pocket "was designed to run optimally with a homogenous 1-4 MB object size" [B10] |
| Source | [B2], [B10] |
| What Satchel simplifies | Blocks reach Magic Pocket already compressed and encrypted [B2]. Satchel stores whatever bytes it's given and does neither. B2 writes "4MB". We read it as 4 MiB, the unit our tests use *(our inference)*. An empty file needs no block, so a zero-byte put is rejected *(our inference)* |
| Status / date | Proposed, 2026-10-28 |

### DDL-002: Block API with dedup on put

| Field | Value |
|---|---|
| Satchel decision | `satchel-api` exposes `PUT /blocks/{hash}`, `GET /blocks/{hash}` and `DELETE /blocks/{hash}`. On a put, the API checks the body's SHA-256 against `{hash}` and rejects a mismatch. It then asks the Block Index whether the hash exists. If it does, the put succeeds without writing (dedup) |
| Magic Pocket equivalent | Frontends are the gateway for put and get. A put first checks the Block Index and stops if the block is already stored [B2] |
| Dropbox's stated reasoning | Content addressing means identical data is stored once. The Frontend decides placement so it can "evenly distribute cell load and minimize network traffic between storage clusters" [B2] |
| Source | [B2] |
| What Satchel simplifies | HTTP and JSON instead of Dropbox's internal RPC. One API instance in the measured setup, where Dropbox runs many Frontends. Status codes are set by the test strategy (#31) |
| Status / date | Proposed, 2026-10-28 |

### DDL-003: Block Index and Replication Table in one relational store

| Field | Value |
|---|---|
| Satchel decision | `satchel-index` is a small HTTP service over one SQLite database with two tables. **Block Index:** `hash → volume, offset, length`. **Replication Table:** `volume → nodes, state (open or closed), type (replicated or RS(6,3)), generation` |
| Magic Pocket equivalent | The Block Index is "a giant sharded MySQL cluster, fronted by an RPC service layer", mapping `hash → cell, bucket, checksum`. The Replication Table maps `bucket → volume` and `volume → OSDs, open, type, generation`, also in MySQL [B2] |
| Dropbox's stated reasoning | Dropbox already ran thousands of MySQL nodes. Databases "are highly reliable and provide an expressive data model", and the choice was "a really great decision in terms of simplifying development and minimizing unknowns" [B2]. Index tables are also easy to scan for verification [B3] |
| Source | [B2], [B3] |
| What Satchel simplifies | One unsharded database instead of a sharded cluster. SQLite instead of MySQL, keeping the reasoning (a proven relational store rather than a custom key-value store) without running a database server *(our inference)*. The index stores the block's offset in its volume, so the Frontend knows which fragment holds it. Magic Pocket stores cell and bucket and leaves offsets to the OSD [B2][B4]. No cells, so no cell column |
| Status / date | Proposed, 2026-10-28 |

### DDL-004: Buckets and volumes of 64 MiB, open then closed

| Field | Value |
|---|---|
| Satchel decision | Blocks are appended to **volumes**. A volume holds one bucket and closes when it reaches 64 MiB (configurable) or has been open for 10 minutes. Only open volumes take writes. Closed volumes are immutable and can be moved or encoded. Each Frontend writes only to the open volumes it was given, so one writer appends to each volume |
| Magic Pocket equivalent | Blocks are grouped into buckets of 1 GB (2016) [B2], later 1 to 4 GB [T9]. A volume holds one or more buckets on a set of OSDs. Open volumes take writes. Closed volumes are immutable "and may be safely moved around the cell" [B2] |
| Dropbox's stated reasoning | 4 MB is "too small a unit of granularity to move around whenever we need to replace a disk or erasure code some data" [B2]. Splitting open from closed keeps live traffic away from background work and "allows us to use far simpler consistency protocols" [B2] |
| Source | [B2], [T9] |
| What Satchel simplifies | 64 MiB instead of 1 to 4 GB, so a lab-sized dataset (tens of GiB) still fills hundreds of volumes and the encoding path is exercised often *(our inference)*. One bucket per volume, no bucket merging. The 10-minute close timer is ours: Dropbox hasn't published when volumes close |
| Status / date | Proposed, 2026-10-28 |

### DDL-005: Three AWS availability zones, one cell

| Field | Value |
|---|---|
| Satchel decision | Satchel's zones are the three availability zones of ap-south-1 (1a, 1b, 1c), simulated locally as `z1`, `z2` and `z3` in Docker Compose. The whole cluster is a single cell |
| Magic Pocket equivalent | Each block is stored "in at least two separate zones" [B2], in data centers "thousands of miles apart" [B5], with three regions in North America [T8]. Each zone holds cells of about 50 PB (2016) [B2], later 100 to 120 PB [T9] |
| Dropbox's stated reasoning | Zones protect against disasters and large outages and give "very clear administrative domains", so a misconfiguration can't cascade [B2]. Different software versions per region are a last line of defence [B5]. Cells give "greater isolation to avoid cascading failures" [B2] |
| Source | [B2], [B5], [T8], [T9] |
| What Satchel simplifies | AZs in one region are kilometres apart, not thousands of miles, so they don't protect against a regional disaster *(our inference)*. One cell instead of many. Every zone runs the same software version |
| Status / date | Proposed, 2026-10-28 |

### DDL-006: Three synchronous replicas, one per zone, for new data

| Field | Value |
|---|---|
| Satchel decision | An open volume has 3 replicas on 3 nodes, one in each zone. The Frontend writes each block to all 3 and acknowledges the put only when all 3 have fsynced. If any write fails, it retries the block on another open volume. If a zone is down, the Master opens volumes with 3 replicas spread over the 2 healthy zones, so writes continue |
| Magic Pocket equivalent | New blocks are replicated: open volumes are 4x replicated within a zone, in two zones [T8][T5][T9]. The Frontend writes to every OSD in the volume, each fsyncs before replying, and a failure means a retry on another volume. A daemon copies each put to the other zone asynchronously, within about a second [B2] |
| Dropbox's stated reasoning | A quorum write might lower tail latency "but at the expense of greater complexity". Retries with careful timeouts "already results in low tail latencies" [B2]. Recent data is replicated before it's coded [B2][B5] |
| Source | [B2], [B5], [T5], [T8], [T9] |
| What Satchel simplifies | 3 copies in total instead of 4 per zone in 2 zones. The cross-zone copy is synchronous instead of an async daemon: Dropbox's zones are far apart, while AZ round trips are short, so a synchronous write should cost little *(our inference, checked by experiment E1 in the exercise plan)*. Keeps the "write to all, no quorum, retry elsewhere" rule |
| Status / date | Proposed, 2026-10-28 |

### DDL-007: RS(6,3) erasure coding with three fragments per zone

| Field | Value |
|---|---|
| Satchel decision | Closed volumes are encoded with Reed-Solomon RS(6,3): 6 data fragments and 3 parity fragments on 9 different nodes, 3 per zone. Any 3 fragments can be lost, so a whole zone can be lost. Data fragments are contiguous slices of the volume, so most blocks are read whole from one node |
| Magic Pocket equivalent | In 2024 Dropbox named RS(6,3) and LRC(12,2,2) as its production codes [T9]. RS(6,3) has 1.5x overhead [T8]. The coding lets "most reads only hit a single disk spindle", with reconstruction on failure [B2] |
| Dropbox's stated reasoning | Erasure coding gives "the same level of fault tolerance as replication, but with significantly less additional storage" [B15]. Volumes are coded "for storage efficiency" [B2] |
| Source | [B2], [B15], [T8], [T9] |
| What Satchel simplifies | Plain Reed-Solomon, not LRC, so rebuilding one lost fragment reads 6 others instead of a smaller local group *(our inference)*. No cross-region cold tier with XOR parity [B5]. The 2024 parameters are used even though the 2016 ones were never published [B3]. Placing 3 fragments per zone so a zone loss is survivable is our layout *(our inference)*; Dropbox hasn't published fragment placement |
| Status / date | Proposed, 2026-10-28 |

### DDL-008: Closed volumes are erasure coded in the background

| Field | Value |
|---|---|
| Satchel decision | When a volume closes, the Master schedules an encode job. A storage node acting as Volume Manager reads one replica, writes the 9 fragments, and the Master commits by bumping the volume's generation and switching its type in the Replication Table. The 3 replicas are then deleted. Encoding can be turned off for experiments that need replicated data only |
| Magic Pocket equivalent | Closed volumes are aggregated and erasure coded in the background [B2][B5]. Volume Managers do the heavy work and run on the storage machines "to amortize their heavy network-capacity demands across idle storage hardware" [B2]. By 2026 a Live Coder writes some data straight into coded volumes [B15] |
| Dropbox's stated reasoning | Keeps background work off the live write path and uses idle storage capacity [B2]. Coding saves space without losing fault tolerance [B15] |
| Source | [B2], [B5], [B15] |
| What Satchel simplifies | No Live Coder. Encoding starts as soon as a volume closes, because Dropbox hasn't published how long volumes stay replicated (architecture notes, section 6) |
| Status / date | Proposed, 2026-10-28 |

### DDL-009: Master duties run off the data path

| Field | Value |
|---|---|
| Satchel decision | A `master` module in the `satchel-verifier` process opens and closes volumes, schedules encode and repair jobs, and watches node health. It keeps no state of its own: everything it needs is in the Replication Table. Puts and gets never go through it |
| Magic Pocket equivalent | One Master per cell, the cell's "janitor or coordinator": it watches OSDs, triggers repair, creates buckets and runs garbage collection. Its state is soft, so it is "not on the data plane" and the cell keeps serving without it [B2] |
| Dropbox's stated reasoning | A "centralized point of coordination for complex data-placement decisions without the significant complexity of a distributed protocol" [B2]. Magic Pocket "eschews quorum-style consensus or distributed coordination as much as possible" [B2] |
| Source | [B2] |
| What Satchel simplifies | The Master shares a process with the verifier instead of being its own service, to keep the service count down *(our inference)*. No bucket merging or garbage collection |
| Status / date | Proposed, 2026-10-28 |

### DDL-010: Repair after a configurable offline threshold

| Field | Value |
|---|---|
| Satchel decision | A node unreachable for longer than `SATCHEL_REPAIR_AFTER` (default 15 minutes, 30 seconds in tests and experiments) is treated as failed. The Master closes its open volumes, rebuilds each missing replica or fragment on another node, preferring the same zone, and commits each move by bumping the volume's generation |
| Magic Pocket equivalent | Repair starts after an OSD has been offline for 15 minutes. The Master closes the OSD's volumes, plans reconstruction across many OSDs, Volume Managers copy and encode, and the move commits through the volume's generation number [B2] |
| Dropbox's stated reasoning | 15 minutes is long enough to restart a node without needless repair, and short enough to keep the window of vulnerability small [B2]. Generation numbers keep moves consistent if anything fails partway [B2] |
| Source | [B2] |
| What Satchel simplifies | No Reconcile process to roll back failed repairs [B2]. Reconstruction runs on one node per volume, not spread across hundreds. The short test threshold is ours, so failure tests finish in minutes |
| Status / date | Proposed, 2026-10-28 |

### DDL-011: Storage nodes store extents as files and fsync before acknowledging

| Field | Value |
|---|---|
| Satchel decision | Each `satchel-node` stores one append-only file per extent (a replica or a fragment of one volume) under its data directory, and fsyncs before acknowledging a write. Each block record carries its hash and length inline, so the node rebuilds its in-memory index by scanning its extents on startup. Nodes know nothing about the cluster topology |
| Magic Pocket equivalent | OSDs are "dumb" nodes that store blocks but "don't understand the cell topology or participate in distributed protocols" [B2]. One daemon owns each disk, which is used raw, without a filesystem [B4][B7]. Since 2022 block metadata is stored inline in the extent and the index is rebuilt on startup [B11] |
| Dropbox's stated reasoning | Keeping nodes simple keeps the protocols simple [B2]. Writing to raw SMR disks gives the OSD control over layout [B4]. Inline metadata let Dropbox remove the SSD write cache, which had capped write throughput [B11] |
| Source | [B2], [B4], [B7], [B11] |
| What Satchel simplifies | Files on a normal filesystem on an EBS volume instead of raw SMR disks. One process per node rather than one per disk |
| Status / date | Proposed, 2026-10-28 |

### DDL-012: Verifier implements the Disk Scrubber and Metadata Scanner

| Field | Value |
|---|---|
| Satchel decision | `satchel-verifier` runs two checks on a loop. **Scrubber:** each node re-hashes every stored block and fragment and reports mismatches. **Metadata scan:** every Block Index entry is checked against the nodes that should hold it. Findings are logged and exported as metrics (#21) |
| Magic Pocket equivalent | Pocket Watch runs six verifiers, including the Disk Scrubber (every bit against checksums, every 1 to 2 weeks) and the Metadata Scanner (every Block Index entry on the right storage nodes, about a million blocks a second) [B3] |
| Dropbox's stated reasoning | Durability depends on constant verification. "More than 50% of the workload on our disks and databases is actually our own internal verification traffic" [B3] |
| Source | [B3] |
| What Satchel simplifies | Two of the six verifiers. No Trash Inspector, Extent Referee, Storage Watcher or Cross-zone Verifier [B3]. Findings are reported, and repair is left to the Master (DDL-010) |
| Status / date | Proposed, 2026-10-28 |

### DDL-013: Deletes remove the index entry, no compaction

| Field | Value |
|---|---|
| Satchel decision | `DELETE /blocks/{hash}` removes the Block Index entry, after which the block can't be read. The bytes stay in their volume. Satchel doesn't reclaim the space |
| Magic Pocket equivalent | Deletes don't free space straight away. Garbage collection marks unreferenced blobs, and compaction copies live blobs into new volumes and retires the old ones. Deleted extents go to a trash area and are only unlinked after inspection [B3][B15] |
| Dropbox's stated reasoning | Closed volumes are immutable, so space comes back only through compaction, which Dropbox rate-limits and keeps within a cell [B15] |
| Source | [B3], [B15] |
| What Satchel simplifies | No garbage collection, compaction or trash. Satchel has no file layer, so it doesn't count references: deleting a deduplicated block deletes it for every writer. Experiments don't delete, so this doesn't affect the measurements *(our inference)* |
| Status / date | Proposed, 2026-10-28 |

### DDL-014: Satchel is written in Rust

| Field | Value |
|---|---|
| Satchel decision | Every Satchel service is written in Rust (stable, 1.91 or later) |
| Magic Pocket equivalent | Magic Pocket moved from a Python prototype to Go, then to Rust for the storage nodes [T5][T3] |
| Dropbox's stated reasoning | "Moving from Go to Rust also allowed us to handle more disks, and larger disks, without increased CPU and Memory costs by being able to directly control memory allocation and garbage collection" [B4] |
| Source | [B4], [T3], [T5] |
| What Satchel simplifies | Nothing. This was decided in the brief and is logged here for completeness |
| Status / date | Proposed, 2026-10-28 |

### DDL-015: Graviton EC2 instances for measured runs

| Field | Value |
|---|---|
| Satchel decision | Measured runs use non-burstable Graviton (arm64) instances in ap-south-1: `m7g.medium` storage nodes with a gp3 EBS data volume, one `m7g.large` for the API, Block Index and verifier, and a `c7g.large` load generator. The Docker image is built for arm64 (#34) |
| Magic Pocket equivalent | **None (deliberate deviation).** Magic Pocket runs on Dropbox's own storage servers [B4][B9][B14] |
| Dropbox's stated reasoning | Not applicable |
| Source | None yet. Instance specifications and hourly prices go into the Source Log before the budget is set (#33) |
| What Satchel simplifies | Cloud VMs with network block storage stand in for dense storage servers. Non-burstable types are chosen so CPU credits can't throttle a run and skew the latency percentiles *(our inference)* |
| Status / date | Proposed, 2026-10-28 |
