# Design decisions: how Satchel mirrors Magic Pocket

Chapter for the final report (#28), from the Design Decision Log (#17, [`decision-log.md`](../../design/decision-log.md)). Issue #27.

Every Satchel decision mirrors a decision Dropbox made public, or is marked as a deliberate deviation. The log has 19 entries. 16 have a Magic Pocket counterpart with a cited reason. 3 are deliberate deviations with no counterpart: the EC2 instance types (DDL-015), running the experiments locally (DDL-016) and the metrics stack (DDL-017). Source IDs refer to the [Source Log](../../sources/README.md).

## 1. Blocks and the block API

Magic Pocket stores immutable blocks of up to 4 MB, normally keyed by their SHA-256 hash, and keeps mutation in a layer above it [B2]. Dropbox put it simply: "Immutability makes our lives a lot easier" [B2]. Satchel keeps the same model: 1 byte to 4 MiB, keyed by SHA-256, never changed after it's written (DDL-001).

The frontend's job follows from that. A put is checked against its key, deduplicated through the Block Index, and stored once (DDL-002), as Magic Pocket's Frontends check the index before writing [B2]. Deletes only remove the index entry (DDL-013). Magic Pocket reclaims space through garbage collection and compaction [B3][B15], which Satchel leaves out because the experiments never delete.

## 2. Metadata: the Block Index and the Replication Table

Magic Pocket keeps two maps in MySQL: the Block Index (hash to cell and bucket) and the Replication Table (volume to storage nodes) [B2]. Dropbox chose MySQL because it already ran thousands of nodes and wanted to minimise unknowns [B2]. Satchel keeps both tables in one relational store, SQLite behind a small HTTP service (DDL-003). The reasoning carries over (a proven relational store rather than a new key-value system) without running a database server.

The index also enforces the open and closed split: it refuses new blocks on a closed volume (DDL-019). In Magic Pocket only open volumes take writes, which "allows us to use far simpler consistency protocols" [B2].

## 3. Volumes, zones and the Master

Single blocks are too small to move or code one at a time, so Magic Pocket groups them into buckets and volumes of 1 GB and up [B2][T9]. Satchel uses 64 MiB volumes that close after 64 MiB or 10 minutes (DDL-004). The smaller size means a lab-sized dataset still fills hundreds of volumes, so the coding path runs often.

Magic Pocket spreads data over zones thousands of miles apart and splits each zone into cells [B2][B5]. Satchel maps its zones to the 3 availability zones of ap-south-1 and runs one cell (DDL-005). This is the largest simplification: AZs are kilometres apart, so they don't protect against a regional disaster *(our inference)*.

Placement and background work belong to a Master that stays off the data path, as in Magic Pocket, where the Master's state is soft and the cell keeps serving without it [B2]. Satchel runs the Master inside the verifier process to keep the service count down (DDL-009).

## 4. Durability: replication first, erasure coding later

New data in Magic Pocket is replicated, with every replica fsynced before the put succeeds and a retry on another volume if one fails. Dropbox chose this over quorum writes, because retries "already results in low tail latencies" without the extra complexity [B2]. Satchel writes 3 synchronous replicas, one per zone, under the same rule (DDL-006). The cross-zone copy is synchronous rather than asynchronous, because AZ round trips are short *(our inference)*.

Closed volumes are erasure coded in the background, as in Magic Pocket [B2][B5] (DDL-008). Satchel uses RS(6,3), one of Dropbox's 2024 production codes [T9], with 1.5x overhead [T8] (DDL-007). Its 9 fragments sit 3 per zone, so losing a whole zone loses exactly the 3 fragments the code can rebuild *(our inference)*. Data fragments are contiguous slices of the volume, so most reads hit one node, as Magic Pocket's reads hit one disk [B2].

## 5. Storage nodes

Magic Pocket's OSDs are deliberately simple: they store blocks but "don't understand the cell topology or participate in distributed protocols" [B2]. Since 2022 they keep each block's metadata inline in the extent and rebuild their index on startup [B11]. Satchel's nodes do the same with append-only files on a normal filesystem, fsynced before every acknowledgement (DDL-011). Record headers stay inside the node, so an offset counts block bytes only and means the same thing on every replica (DDL-018).

## 6. Repair and verification

Magic Pocket repairs an OSD that has been offline for 15 minutes: long enough to restart without needless work, short enough to keep the window of vulnerability small [B2]. Each move commits by bumping the volume's generation [B2]. Satchel keeps both rules, with the threshold configurable so tests can use 30 seconds (DDL-010).

Dropbox says more than half the load on its disks and databases is its own verification [B3]. Satchel implements two of Pocket Watch's six verifiers: the Disk Scrubber and the Metadata Scanner (DDL-012).

## 7. Implementation and deployment

Satchel is written in Rust, following Dropbox's move from Go to Rust for its storage nodes [B4][T3] (DDL-014). The rest are deliberate deviations, chosen for a course project:

- **Experiments run on the local Compose cluster** (DDL-016). AWS comes up only for the live demo and a short S3 comparison, to keep spend near zero. So local results show relative costs, not network latency between AZs.
- **The demo uses Graviton instances** (DDL-015), priced from the AWS calculator [P3].
- **Metrics come from Prometheus and Grafana** (DDL-017). Labels use route patterns, so block hashes never become label values.

## Summary

| ID | Satchel decision | Magic Pocket equivalent | What Satchel simplifies |
|---|---|---|---|
| DDL-001 | Immutable blocks, 1 B to 4 MiB, keyed by SHA-256 | Immutable blocks up to 4 MB, SHA-256 [B2] | No compression or encryption |
| DDL-002 | Put, get, delete, with dedup on put | Frontends check the index before writing [B2] | HTTP and JSON, one frontend |
| DDL-003 | Block Index and Replication Table in SQLite | Sharded MySQL [B2] | One unsharded database, offsets in the index |
| DDL-004 | 64 MiB volumes, open then closed | 1 to 4 GB buckets, open and closed volumes [B2][T9] | Smaller volumes, our own close timer |
| DDL-005 | 3 AZs, one cell | Zones thousands of miles apart, many cells [B2][B5] | No regional disaster protection |
| DDL-006 | 3 synchronous replicas, one per zone | Write to every replica, retry elsewhere [B2] | 3 copies instead of 8, synchronous cross-zone |
| DDL-007 | RS(6,3), 3 fragments per zone | RS(6,3) or LRC(12,2,2) [T9] | Plain Reed-Solomon, no cold tier |
| DDL-008 | Code closed volumes in the background | Background coding [B2][B5] | No Live Coder |
| DDL-009 | Master off the data path | Soft-state cell Master [B2] | Shares the verifier's process |
| DDL-010 | Repair after a configurable threshold | Repair after 15 minutes offline [B2] | No Reconcile, one node per volume |
| DDL-011 | Append-only extent files, fsync, inline metadata | Raw SMR disks, inline metadata [B4][B11] | Files on a filesystem |
| DDL-012 | Disk Scrubber and Metadata Scanner | Six Pocket Watch verifiers [B3] | Two of six |
| DDL-013 | Delete removes the index entry | Garbage collection and compaction [B3][B15] | No space reclaimed |
| DDL-014 | Rust | Go, then Rust, for OSDs [B4][T3] | Nothing |
| DDL-015 | Graviton instances for the demo | None (deviation) | Cloud VMs instead of storage servers |
| DDL-016 | Experiments on Compose, AWS for the demo only | None (deviation) | No measurement on separate hardware |
| DDL-017 | Prometheus metrics by route pattern | None (deviation) | Estimated dashboard percentiles |
| DDL-018 | Offsets count block bytes only | OSDs own their layout [B2][B11] | Offsets in the index |
| DDL-019 | No new blocks on closed volumes | Only open volumes take writes [B2] | Checked in the index transaction |

## Before the report is submitted

- Every entry is still **Proposed**. They become **Accepted** when the Decision Log (#55) and the exercise plan (#56) merge. Update this chapter's wording then.
- Check the build matches the log once the Week 3 PRs merge. In review, the Master (#92) opened 2-replica volumes when a zone was down, where DDL-006 says 3 replicas over 2 zones. If that stays, it needs a log entry as a deviation.
