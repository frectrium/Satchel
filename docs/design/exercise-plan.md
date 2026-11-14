# Exercise plan

Issue #14. What we build in Week 3, where it runs, and what we measure. This turns the [project brief](../SATCHEL_PROJECT.md) into numbers. Each design decision here is recorded in full, with its Magic Pocket counterpart and sources, in the [Design Decision Log](decision-log.md) (#17). This doc gives the `DDL-NNN` ID instead of repeating it.

Source IDs refer to the [Source Log](../sources/README.md). Anything we work out ourselves is marked *(our inference)*.

## 1. Decisions at a glance

| Question from #14 | Decision | DDL |
|---|---|---|
| Block size and hash | 1 byte to 4 MiB, keyed by SHA-256, immutable. Empty blocks rejected | DDL-001 |
| API | `PUT`, `GET`, `DELETE /blocks/{hash}`, hash checked on put, dedup through the Block Index | DDL-002 |
| Index | One SQLite database behind `satchel-index`, holding the Block Index and the Replication Table | DDL-003 |
| Unit of placement | Volumes of 64 MiB (one bucket each). Open volumes take writes, closed ones are immutable | DDL-004 |
| Zones | ap-south-1a, 1b, 1c (locally `z1`, `z2`, `z3`). One cell | DDL-005 |
| Replication factor and placement | 3 synchronous replicas per open volume, one per zone. All 3 must fsync before the put succeeds | DDL-006 |
| Erasure coding scheme | **RS(6,3)**: 6 data + 3 parity fragments on 9 nodes, 3 per zone. Survives the loss of any 3 nodes, or a whole zone | DDL-007 |
| When data moves from replicated to coded | When a volume closes (64 MiB, or 10 minutes open). Encoding runs in the background | DDL-008 |
| Coordination | A Master module in the verifier process, off the data path | DDL-009 |
| Repair | After `SATCHEL_REPAIR_AFTER` offline: 15 min by default, 30 s in tests and experiments | DDL-010 |
| Storage node | Append-only extent files, fsync before ack, inline metadata | DDL-011 |
| Verifier | Disk Scrubber and Metadata Scanner equivalents | DDL-012 |
| Deletes | Remove the index entry, no compaction | DDL-013 |
| Nodes per zone | 3 in development and in the base measured setup (9 in total). 4 and 6 per zone in the scaling experiment | Section 3 |
| EC2 instance types | `m7g.medium` nodes, `m7g.large` control instance, `c7g.large` load generator | DDL-015 |
| Comparison with S3 | Same load generator and workload against S3 Standard in ap-south-1 | Section 6 |

### Why RS(6,3) and 9 nodes

- RS(6,3) is one of Magic Pocket's two production codes in 2024 [T9], with 1.5x storage overhead [T8].
- 9 fragments need 9 different nodes. With 3 nodes in each of 3 zones, each zone holds exactly 3 fragments. Losing a zone loses 3 fragments, which is exactly what RS(6,3) tolerates *(our inference)*.
- LRC(12,2,2), Dropbox's other code [T9], needs 16 fragments, so at least 16 nodes. That's too much EC2 for a 5-week project and harder to implement *(our inference)*.
- The 2016 production code was never published [B3], so we use the 2024 one and say so.

## 2. How the pieces fit

### Put

1. The client sends `PUT /blocks/{hash}` to `satchel-api`.
2. The API checks the body hashes to `{hash}` and that its size is 1 byte to 4 MiB.
3. The API asks `satchel-index` whether the hash exists. If it does, it returns success (dedup).
4. The API picks one of its open volumes and the next offset in it. The Replication Table gives the volume's 3 nodes.
5. The API writes the block to all 3 nodes at that offset. Each node appends and fsyncs before replying.
6. If any node fails, the API retries the block on another open volume.
7. The API records `hash → volume, offset, length` in the Block Index and returns success.

### Get

1. The API looks up the hash in the Block Index, then the volume in the Replication Table.
2. **Replicated volume:** read from one replica, preferring the API's own zone. On failure, try the next replica.
3. **Coded volume:** the offset gives the data fragment that holds the block. Read it from that one node. If the block crosses a fragment boundary, read from the two nodes involved.
4. **Degraded read:** if a needed node is down, read any 6 of the remaining fragments for that range and decode.
5. The API checks the returned bytes hash to `{hash}` before replying.

### Background work (Master, in the verifier process)

- Keeps each API supplied with open volumes, and closes volumes that are full or past the timer.
- Schedules an encode job for each closed replicated volume. A node holding a replica acts as Volume Manager, writes the 9 fragments, and the Master commits the new layout by bumping the volume's generation. The old replicas are then deleted.
- Watches node health and runs repair (DDL-010).
- If a zone is down, opens new volumes with 3 replicas spread over the 2 healthy zones, so writes continue (DDL-006).

## 3. Environments

### Development: Docker Compose

| Service | Count | Notes |
|---|---|---|
| `api` | 1 | Port 8080 |
| `index` | 1 | Port 8081, SQLite file on a named volume |
| `verifier` (with the Master) | 1 | Port 8085 |
| `node` | **9**: `node-z1-1` to `node-z3-3` | 3 per zone, host ports 8091 to 8099, one named volume each |
| `prometheus`, `grafana` | 1 each | Added by #21 |

Compose currently has 3 nodes. Growing it to 9 is part of the storage node build issue. The integration harness (#32) starts the same 9-node cluster.

### Measurement: AWS ap-south-1

| Role | Count | Instance | Placement |
|---|---|---|---|
| Storage node | 9 (base). 12 and 18 in experiment E2 | `m7g.medium`, one gp3 EBS data volume each (50 GiB) | Spread evenly over 1a, 1b, 1c |
| API, Block Index, verifier, Prometheus | 1 | `m7g.large` | 1a |
| Load generator | 1 | `c7g.large` | 1a, same AZ as the API so the client-to-API hop is the same for every run |
| S3 bucket (baseline) | 1 | S3 Standard | ap-south-1 |

- All instances run the arm64 Docker image, pulled by commit SHA (#34).
- 50 GiB per node covers the largest dataset in section 4 (50 GiB logical, 150 GiB raw while replicated, spread over 9 nodes) with room to spare *(our calc)*.
- A gp3 volume has a fixed baseline of IOPS and throughput. If a node's disk becomes the bottleneck, the results say so instead of hiding it.
- Instance specifications and hourly prices are not in the Source Log yet. #33 adds them before the budget is set, and DDL-015 is updated.
- Every session ends with `terraform destroy` (#20).

## 4. Experiments

Each experiment runs from the load generator VM, never a laptop. Every configuration is repeated **3 times**. We report the median of the 3 runs and their range.

**Run length:** each run measures at least 10,000 operations, so p99.9 rests on at least 10 samples, or stops at 5 minutes, whichever comes first. The sample count goes in the results. Each run starts with 30 seconds of warm-up that isn't measured.

**Defaults** unless a row says otherwise: 9 nodes, 16 concurrent clients, 2 MiB blocks (the workload model's average, #15), and a get-to-put ratio of 2:1, matching the workload model's egress of twice the ingest (#15).

### E1. Latency vs block size, Satchel vs S3

| Factor | Values |
|---|---|
| Block size | 4 KiB, 64 KiB, 256 KiB, 1 MiB, 2 MiB, 4 MiB |
| Operation | Put. Get from a replicated volume. Get from a coded volume (Satchel only) |
| Concurrency | 1 and 16 clients |
| Target | Satchel and S3 |
| Metrics | p50, p90, p99, p99.9 latency, throughput (ops/s and MiB/s), errors |

This also tests DDL-006: if synchronous cross-zone writes were expensive, Satchel's put latency would show it against S3.

### E2. Scaling

Change one factor at a time from the defaults.

| Factor | Values |
|---|---|
| Storage nodes | 9, 12, 18 (3, 4 and 6 per zone) |
| Concurrent clients | 1, 4, 16, 64 |
| Data already stored | 1 GiB, 10 GiB, 50 GiB |
| Metrics | Throughput, latency percentiles, per-node CPU, memory, disk and network (#21) |

### E3. Replication vs erasure coding

| Measure | How |
|---|---|
| Storage overhead | Bytes on disk across all nodes divided by logical bytes stored, for 10 GiB written with encoding off and then on. Expected 3.0x and 1.5x *(our calc)* |
| Read latency | Gets of 2 MiB blocks from replicated volumes, healthy coded volumes, and coded volumes with one data-fragment node stopped (degraded reads) |
| Encoding cost | Time and CPU to encode 10 GiB of closed volumes. MiB/s per Volume Manager |
| Write latency | Puts always go to replicated volumes. Check that encoding running in the background doesn't raise put latency |

### E4. Node and zone failure

| Scenario | How | Measure |
|---|---|---|
| One node stopped | Stop one node's container during a steady run | Error rate and latency while it's down. Time to detect, time to repair, and GiB repaired (repair threshold 30 s) |
| Node returns before the threshold | Stop a node for 15 s, then start it again | No repair is triggered |
| Zone lost | Stop all 3 nodes in one zone | Reads of replicated and coded data still succeed. Writes continue on volumes placed in the two healthy zones (DDL-006) |
| Corrupted block | Flip a byte in a stored extent | The scrubber reports it, and a get still returns correct data (#31) |

### E5. Satchel's own resource usage

A 30-minute steady run at the defaults. Record CPU, memory, disk and network for every instance, and bytes stored per node. From this we compute what Satchel costs per GB-month stored and per million requests on EC2, which feeds the Satchel-on-EC2 scenario in the cost engine (#52) and the cross-check in #26.

### Budget in instance-hours

Our rough count, before prices are sourced *(our calc)*:

| Experiment | Configurations | Runs (x3) | Time per run | Approximate hours of the cluster |
|---|---:|---:|---|---:|
| E1 | 36 Satchel + 24 S3 | 180 | 2 to 5 min | 9 |
| E2 | 8 | 24 | 5 min | 2 to 3 (18-node runs cost more) |
| E3 | 4 | 12 | 5 to 10 min | 2 |
| E4 | 4 | 12 | 10 min | 2 |
| E5 | 1 | 3 | 30 min | 1.5 |

That's about 17 hours of an 11-instance cluster, spread over several sessions, plus setup and teardown. #35 measures what one session really costs. If that's higher than the budget allows, we cut E1 repeats first, then E2's 18-node runs.

## 5. What each experiment needs from the build

| Need | Provided by |
|---|---|
| Per-request latency histograms, throughput, errors, bytes per node | Observability (#21) |
| Load generator with block size, mix, concurrency, duration and data volume settings, for Satchel and S3 | Load generator (#22) |
| A switch to turn encoding off, and a short repair threshold | DDL-008, DDL-010 |
| A way to stop and start a node, and to corrupt a block | Compose and EC2 instance controls, test harness (#32) |
| A cluster with 9, 12 or 18 nodes | Terraform variable (#20) |

## 6. Comparing against S3

- **Same client:** one load generator binary (#22) with an `s3` backend, run from the same VM.
- **Same workload:** the same block sizes, contents, mix and concurrency. The S3 object key is the block's hash.
- **Same region:** an S3 Standard bucket in ap-south-1. S3 Standard keeps data in at least 3 AZs [P2], like Satchel. #20 decides how the load generator reaches S3 inside the region and records it, since the path changes latency.
- **What's compared:** latency percentiles and throughput (E1), and cost per GB-month and per request: S3's list prices [P1] against Satchel's measured cost on EC2 (E5).
- **What's not comparable:** S3 is a managed service whose internal design isn't published beyond its AZ count and durability target [P2]. Satchel's numbers come from 9 small VMs. The comparison shows how a self-run store behaves next to the service Dropbox left, not which design is better *(our inference)*.

## 7. Cost model and calculator

The brief asks for the workload to be priced on AWS and Azure and compared with on-prem. The course also expects a programmatic calculator. The pieces:

| Piece | Issue |
|---|---|
| Workload (the case, priced at Dropbox's scale) | #15 |
| AWS and Azure calculator estimates | #23, #24 |
| On-prem estimate | #25 |
| Comparison and break-even | #26 |
| Cost engine and CLI (`satchel-cost`) | #52 |
| Sizing model: blocks, volumes, drives, servers, racks, cloud tiers | #53 |
| Interactive calculator web UI | #54 |

**Which redundancy to price.** The case workload models Dropbox, so the on-prem estimate uses Magic Pocket's layout as #15 sets it: two geographic copies, each RS(6,3), for 3.0x raw [B2][T8][T9]. Satchel's own layout (3x replicated, then 1.5x coded) is only used for the Satchel-on-EC2 scenario priced from E5. #25 should take its overhead from #15, not from this plan.

## 8. Values for other issues

| Issue | Value from this plan |
|---|---|
| Test strategy (#31) | Replication: 3 replicas, one per zone. Erasure coding: k = 6, m = 3. Repair threshold: 30 s in tests. Empty blocks: rejected with 400. Zone loss is tolerated, so the zone-loss failure test applies |
| Integration harness (#32) | 9 storage nodes, 3 per zone |
| AWS groundwork (#33) and Terraform (#20) | Instance types and counts in section 3. Node count is a variable: 9, 12 or 18 |
| Docker image (#34) | arm64 |
| Observability (#21) | Metrics listed in section 5 |
| Load generator (#22) | Factors and defaults in section 4 |
| On-prem estimate (#25) | Overhead from #15 (section 7) |

## 9. Week 3 build and experiment issues

#19 creates these once this plan is merged. Owners are split evenly in #19 and recorded in the Team Roles doc (#16).

| Issue | Covers | DDL |
|---|---|---|
| Block Index and Replication Table | `satchel-index`, SQLite schema, lookups for the API and Master | DDL-003 |
| Storage node | Extent files, fsync, inline metadata, index rebuild, 9-node Compose | DDL-011 |
| Frontend: put, get, delete | `satchel-api`, hash check, dedup, replicated writes and reads | DDL-001, 002, 006, 013 |
| Master: volumes and placement | Open and close volumes, zone-aware placement | DDL-004, 005, 009 |
| Erasure coding | RS(6,3) encode, fragment placement, coded and degraded reads | DDL-007, 008 |
| Repair | Failure detection and rebuild of replicas and fragments | DDL-010 |
| Verifier | Scrubber and metadata scan | DDL-012 |
| Horizontal scaling | Adding and removing nodes, new volumes placed on new nodes | DDL-005 |
| Experiment E1 | Latency vs block size against S3 | |
| Experiment E2 | Scaling | |
| Experiment E3 and E4 | Replication vs erasure coding, failures | |
| Experiment E5 | Resource usage for the cost model | |

## 10. Decisions to confirm in review

- RS(6,3) on 9 nodes, and 3 synchronous replicas before encoding (DDL-006, DDL-007).
- SQLite for the index instead of MySQL (DDL-003).
- 64 MiB volumes and the 10-minute close timer (DDL-004).
- The Master living in the verifier process (DDL-009).
- The instance types (DDL-015) and the size of the experiment matrix in section 4.
