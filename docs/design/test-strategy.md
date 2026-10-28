# Test strategy

Issue #31. How Satchel is tested, so every Week 3 build issue (#19) knows which tests it has to ship with. Each requirement has an ID (`TS-...`). A build issue links to the IDs it must meet, and is done when those tests pass.

The component list and erasure coding parameters come from the exercise plan (#14). Where #14 hasn't fixed a number yet, this doc says **(set by #14)**. The doc gets updated when #14 merges.

## 1. How Dropbox tests Magic Pocket, and what Satchel copies

| Magic Pocket practice | Source | Satchel equivalent |
|---|---|---|
| Integration tests with failure injection before every release | [B3] | Integration tests on the Compose cluster, including node stops and on-disk corruption (sections 3 and 5) |
| Verifiers are tested by deliberately corrupting data | [B3] | Corruption tests in TS-VER and TS-FAIL |
| Every invariant tested weekly; a full zone taken offline for about 30 minutes each week | [T9] | Failure drills before each experiment run and the release (section 6) |
| Storage Watcher re-reads a sample of writes after a minute, hour, day, week and month | [B3] | Read-back checks in the integration suite (TS-API-3) |

## 2. Test levels

| Level | What runs | Where | When | Owner |
|---|---|---|---|---|
| **Unit** | `#[test]` and `#[tokio::test]` functions next to the code, one module or crate at a time. No network, no Docker | Developer machine and CI | Every PR | Author of the change |
| **Integration** | Tests that drive the six running services over HTTP, as a client would | Local Compose cluster (`docker compose up`), locally and in CI | Every PR | Author of the change. Harness from #32 |
| **End-to-end (AWS)** | The same integration suite pointed at the AWS deployment, plus failure drills | 3-AZ cluster in ap-south-1 (#20, #35) | Before each experiment run and before the release (#36) | Whoever runs the experiment |

Performance is **not** a test level. See section 8.

## 3. Requirements per component

### API (`satchel-api`, Magic Pocket Frontend)

The status codes below are proposals. The API build issue may change them, and this table is updated to match.

| ID | The test shows that |
|---|---|
| TS-API-1 | A block can be put and then read back byte for byte |
| TS-API-2 | The block's key is the SHA-256 of its content (`BlockHash`). A put whose content doesn't match the given hash is rejected |
| TS-API-3 | A block put earlier is still readable after the cluster restarts (`docker compose restart`) |
| TS-API-4 | Putting the same block twice stores it once (dedup) and both puts succeed |
| TS-API-5 | Get or delete of an unknown hash returns 404 |
| TS-API-6 | A malformed hash returns 400. A block over 4 MB returns 413 (4 MB is the Magic Pocket block limit [B2]) |
| TS-API-7 | A deleted block can no longer be read |
| TS-API-8 | When the Block Index is down, put and get fail with a clear 5xx, not a hang (a timeout is set) |

### Block Index (`satchel-index`)

| ID | The test shows that |
|---|---|
| TS-IDX-1 | Insert, lookup and delete of a `hash → locations` entry |
| TS-IDX-2 | Lookup of an unknown hash returns "not found", not an error |
| TS-IDX-3 | Inserting an existing hash doesn't create a duplicate entry |
| TS-IDX-4 | Entries survive an index restart |

### Storage node (`satchel-node`, Magic Pocket OSD)

| ID | The test shows that |
|---|---|
| TS-NODE-1 | Write then read a block or fragment |
| TS-NODE-2 | A write is acknowledged only after it is flushed to disk (fsync), as Magic Pocket OSDs do [B2]. Unit level: the write path calls sync before replying |
| TS-NODE-3 | Data survives a node restart (Docker volume `/data`) |
| TS-NODE-4 | `GET /health` reports the node's zone from `SATCHEL_ZONE` |
| TS-NODE-5 | A read of a block whose bytes no longer match its checksum returns an error, not the bad bytes |

### Replication

| ID | The test shows that |
|---|---|
| TS-REP-1 | After a put, the block is on the required number of nodes **(set by #14)**, each in a different zone |
| TS-REP-2 | A get still succeeds with any one node stopped |

### Erasure coding

Parameters: k data and m parity fragments **(set by #14)**. Magic Pocket's 2024 choices are RS(6,3) and LRC(12,2,2) [T9].

| ID | The test shows that |
|---|---|
| TS-EC-1 | Encode then decode returns the original block, for every size in section 4 |
| TS-EC-2 | Decode succeeds with **any** m fragments missing. The unit test tries every combination of m lost fragments |
| TS-EC-3 | Decode with m + 1 fragments missing returns an error, never wrong data |
| TS-EC-4 | A corrupted fragment is detected by its checksum and treated as missing |
| TS-EC-5 | Integration: a block stays readable with m storage nodes stopped |

### Verifier (`satchel-verifier`, Magic Pocket Pocket Watch)

| ID | The test shows that |
|---|---|
| TS-VER-1 | A block corrupted on disk is reported (Disk Scrubber equivalent [B3]) |
| TS-VER-2 | A block missing from a node that the index lists is reported (Metadata Scanner equivalent [B3]) |
| TS-VER-3 | A clean cluster produces no reports (no false positives) |
| TS-VER-4 | Each finding names the hash and the node, and is counted in a metric for the dashboards (#21) |

### All services

| ID | The test shows that |
|---|---|
| TS-SVC-1 | All six services answer `GET /health` with `status: ok` and their service name. **This is the first integration test (#32)** |
| TS-SVC-2 | Each service reads its port from `SATCHEL_PORT` and falls back to its default |

## 4. Test data

- **Generation:** a seeded pseudo-random generator, so every failure can be reproduced from its seed. Log the seed on failure. No random data without a seed.
- **Block sizes:**

  | Size | Why |
  |---|---|
  | 0 bytes | Edge case. Allowed or rejected **(set by #14)**, but defined |
  | 1 byte | Smallest real block |
  | 4 KiB | Typical disk page |
  | 128 KiB | S3 minimum billable object size for some tiers [P1] |
  | 1 MiB, 2 MiB | Typical Magic Pocket range [B10]. 2 MB is the workload model's average (#15) |
  | 4 MiB | Largest allowed block [B2] |
  | 4 MiB + 1 byte | Must be rejected (TS-API-6) |
  | Not a multiple of k | Erasure coding padding (TS-EC-1) |

- **Duplicates:** the same block content put twice, for TS-API-4.
- **Size of the suite:** the integration suite writes no more than about 100 MB per run, so it stays fast in CI.

## 5. Failure tests

| ID | Failure | How it's injected | Pass when |
|---|---|---|---|
| TS-FAIL-1 | A storage node stops | `docker compose stop node-z2` (on AWS: stop the instance) | Reads still succeed (TS-REP-2, TS-EC-5) |
| TS-FAIL-2 | A node stays down past the repair threshold | Keep it stopped | Missing copies or fragments are rebuilt on other nodes. Magic Pocket waits 15 minutes [B2]; Satchel makes the threshold configurable and uses seconds in tests **(value set by #14)** |
| TS-FAIL-3 | A block is corrupted on disk | Flip one byte in the stored file inside the node container (`docker compose exec`) | The verifier reports it (TS-VER-1) and a get still returns correct data from another copy or from decoding |
| TS-FAIL-4 | A block is deleted from disk | Remove the stored file inside the node container | The verifier reports it (TS-VER-2) |
| TS-FAIL-5 | A whole zone is lost | Stop every node in one zone | Reads still succeed if #14's placement promises zone tolerance. Mirrors Dropbox's weekly zone drill [T9] |

TS-FAIL-1 to TS-FAIL-4 run in CI once the features they test exist. TS-FAIL-5 runs before experiments and the release.

## 6. What runs when

| Trigger | Runs | Time budget |
|---|---|---|
| Every PR and push to `main` | `cargo fmt --check`, `cargo clippy`, unit tests, Docker image build, Compose config check (all already in CI), plus the integration suite on the Compose cluster (#32) | Integration job under 10 minutes |
| Before an experiment run on AWS | Integration suite against the AWS cluster, then TS-FAIL-1 and TS-FAIL-5 | Under 30 minutes, then `terraform destroy` if no experiment follows |
| Before the release (#36) | Everything above, plus a fresh cluster from scratch | |
| Never in CI | Performance experiments (section 8) | |

Integration tests must not run during the normal `cargo test --workspace` in the existing Rust job, which has no cluster. #32 decides how they're kept separate (for example `#[ignore]` plus a dedicated CI job).

## 7. Writing a test

- **Unit tests** go in a `#[cfg(test)] mod tests` block in the same file. Integration tests go in the harness crate from #32.
- **Naming:** name each test after the requirement, for example `ts_api_4_duplicate_put_stores_once`.
- **Independence:** every integration test uses its own fresh blocks (new seed), so tests can run in any order and in parallel.
- **No sleeps for ordering.** Poll with a timeout instead: retry until the condition holds or the timeout expires.
- **Each build issue (#19) lists its TS IDs.** Its PR adds those tests, and CI must pass.

## 8. Tests vs performance experiments

| | Functional tests (this doc) | Performance experiments (#14, #22) |
|---|---|---|
| Question | Is it correct? | How fast, how far does it scale, what does it cost? |
| Result | Pass or fail | Numbers: latency percentiles, throughput, storage overhead, cost |
| Where | Laptop, CI, briefly on AWS | AWS only, with the load generator on a separate VM in the same region |
| Data size | Small (about 100 MB per run) | Large, sized by the experiment plan |
| Gates a PR? | Yes | No. Results go into the report |
| Includes S3? | No | Yes, S3 is the baseline |
