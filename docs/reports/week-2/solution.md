# The solution: how Magic Pocket works

Magic Pocket is Dropbox's custom block store for user file content. It divides files into immutable blocks of up to 4 MB, routes them through a small set of metadata services, stores them on simple disk servers, and continuously checks that every required copy can still be found and read. The design separates the live request path from slower placement, coding, repair, and verification work. [B2][B3]

This account uses the primary-source IDs in the [Source Log](../../sources/README.md). Magic Pocket changed between 2016 and 2026, so each later improvement is dated rather than projected back onto the migration-era system.

## Walking the architecture

The component order below is the order to follow in the Magic Pocket architecture diagram being produced in [issue #11](https://github.com/frectrium/Satchel/issues/11). It starts with the live put and get path, then moves to the background systems that make the data durable.

### 1. Blocks make the storage layer simple

Dropbox services split file content into opaque blocks of up to 4 MB before sending it to Magic Pocket. Blocks are compressed and encrypted before arrival, normally addressed by their SHA-256 hash, and never modified after they are written. File changes are represented in a higher-level service called FileJournal. [B2]

Immutability removes in-place update coordination from the storage layer. The content hash also lets a Frontend check whether a block already exists before writing another copy. This directly supports the need for a predictable storage path at very large scale, although deleting immutable data later requires garbage collection and compaction. [B2][B15]

Blocks are grouped into buckets and volumes. A block is too small to move individually during repair or erasure coding, so the 2016 design grouped blocks into 1 GB buckets. A volume contains one or more buckets and is either open for new writes or closed and safe to move. This open/closed split allowed Dropbox to use simpler consistency protocols while background workers moved larger units efficiently. [B2]

### 2. Frontends route every put and get

A Frontend is the gateway to Magic Pocket. For a put, it first queries the Block Index for an existing hash. If the block is new, it chooses an open volume, asks the Replication Table which storage nodes hold that volume, writes to every listed node, waits for each node to fsync, and then records the block in the Block Index. If a storage node fails, it retries with another volume. [B2]

For a get, the Frontend uses the Block Index to find the cell and bucket, then the Replication Table to find the volume and its storage nodes. It normally reads the complete block from one disk. If that disk is unavailable, the Frontend reconstructs the block from other erasure-coded fragments with help from a Volume Manager. [B2]

Dropbox chose retries and careful timeouts instead of quorum writes on the request path. The company said that this kept tail latency low without adding the complexity of a quorum protocol. That choice addressed the performance requirement while following Magic Pocket's stated principle of avoiding distributed coordination where a simpler mechanism was sufficient. [B2]

### 3. Two metadata systems answer different questions

The **Block Index** maps a block hash to a cell, bucket, and checksum. It is a sharded MySQL deployment behind an RPC service. Dropbox already operated thousands of MySQL nodes, valued their reliability and expressive data model, and could scan the tables for verification. Reusing that operational knowledge reduced the number of new systems the small Magic Pocket team had to invent. [B2][B3]

The **Replication Table** maps buckets to volumes and volumes to storage nodes, type, open state, and generation. It changes less often and is small enough for its working set to fit in memory. Separating block lookup from physical placement lets data move or be repaired without changing every block entry. The generation number also makes a repair commit explicit. [B2]

### 4. Cells contain failures and make growth repeatable

Each geographic zone is divided into cells. A cell was about 50 PB of raw capacity in 2016 and was designed as a self-contained storage cluster spread across racks. Dropbox added capacity by adding another cell. The boundary gave the team a repeatable deployment unit and reduced the chance that a failure would cascade across the whole fleet. [B2]

Each cell has a Master and a set of Volume Managers. The Master watches storage nodes, creates buckets, triggers repair and garbage collection, and makes placement decisions. Its state is recoverable from the Replication Table, so it remains outside the live data path. Reads continue without it, and writes continue while open buckets remain. This central coordinator avoided putting a consensus system into every operation. [B2]

Volume Managers perform the network-heavy work of moving, reconstructing, and erasure-coding volumes. Dropbox ran them on storage hardware so that background work could use otherwise idle network capacity. [B2]

### 5. OSDs keep storage nodes deliberately narrow

Object storage devices, or OSDs, store blocks but do not know the cell topology and do not participate in distributed protocols. One daemon owns each disk. Later designs wrote directly to host-managed shingled magnetic recording disks through a raw zoned interface, avoiding a conventional filesystem and controlling physical data layout. [B2][B4][T7]

This narrow role moved coordination into the metadata and management services while letting Dropbox optimize density and sequential writes. Host-managed SMR offered a better cost per gigabyte, and the immutable extent layout matched its sequential-write constraint. Dropbox also moved the OSD implementation from Go to Rust to control allocation and garbage collection as disk counts and capacities grew. [B4][T3]

### 6. Replication absorbs the first-write risk

New blocks initially land in replicated open volumes. The 2022 account describes four replicas inside each of two zones before coding, for eight copies during the replicated stage. A cross-zone daemon asynchronously copies each put to the other zone, normally within about a second. Independent zones use separate administrative and software domains so that congestion, misconfiguration, or a bad release is less likely to affect every copy. [B2][B5][T8]

This is an intentionally expensive first state. It gives new data a straightforward write and recovery path when it is hottest and most likely to be read, while protecting it across distant failure domains. This connection to the migration's durability requirement is our inference; Dropbox acknowledged that geographic duplication effectively doubled the data stored in the two-region layout. [B2][B5]

### 7. Erasure coding reduces the steady-state cost

After a volume closes, background workers aggregate and erasure-code it. The layout keeps a whole block on one OSD for the normal read path, while enough other fragments remain to reconstruct it after a failure. Dropbox described the 2016 scheme only as a Reed-Solomon variant similar to local reconstruction codes. By 2024, its disclosed production choices were RS(6,3) or LRC(12,2,2); those later parameters should not be treated as the exact 2016 configuration. [B2][B3][T9]

The replicated-first, coded-later lifecycle balances the two pressures behind Magic Pocket: fast and conservative ingestion for fresh data, then lower storage overhead for the long-lived majority. That balance is our inference from the disclosed lifecycle. Dropbox's 2019 cold tier went further by spreading two data fragments and an XOR parity fragment across three regions, reducing cross-region overhead from 2x to 1.5x and saving 25% of disk for cold data. [B5][T8]

### 8. Repair and verification make durability observable

When an OSD remains offline for 15 minutes, the Master closes its volumes and plans reconstruction across hundreds of other nodes. Volume Managers perform the copies or coding, the new placement commits by increasing the volume generation, and a Reconcile process rolls back incomplete repairs. The delay allows ordinary restarts without starting unnecessary fleet-wide work while limiting the vulnerability window. [B2]

Pocket Watch verifies the result from several directions. Disk Scrubber checks stored bytes against checksums; Metadata Scanner checks that Block Index entries have the expected physical copies; Storage Watcher performs sampled black-box reads; and other verifiers check deletion, extent movement, and cross-zone completeness. Dropbox reported that verification generated more workload and code than the core storage path, and it deliberately corrupted staging data to prove the verifiers could detect faults. [B3][T5]

These systems addressed the largest risk in replacing S3: Dropbox now had to prove its own storage remained recoverable. Durability came from independent copies, coding, repair, staged releases, and continuous cross-checks rather than from trusting one component. [B2][B3]

## Migrating while Dropbox stayed live

The production move was staged over more than a year:

| Date | Migration step |
|---|---|
| Summer 2013 | Magic Pocket became a major initiative after a smaller proof of concept. [B1] |
| August 2014 | Dropbox dark-launched the system, mirrored data in two regions, and retained extra backups. [B1] |
| February 2015 | After about six months of validation, selected user files were stored and served exclusively in-house. A staging bug found after about 40 days had reset the validation clock without losing user data. [B1][T5] |
| April to October 2015 | Base Jump expanded servers in three locations while transfers exceeded half a terabit per second and later passed 6 PB per day. [B1][T5] |
| October 2015 | More than 90% of user data was served in-house, ahead of the target and without a major service disruption or data loss during the migration. [B1] |
| Fourth quarter 2016 | The S-1 dates completion of the broader transition period in which data remained duplicated across third-party and custom systems. [F1, p. 67] |

Dropbox migrated users individually rather than switching the whole service at once. It copied a user's blocks, verified the new copies, then removed the old S3 copies. Keeping both systems was expensive, but it preserved a rollback path. The large AWS commitment deadline made speed important, while dark launch, per-user cutover, and verification kept the work reversible. [T4][T5]

The same caution continued after launch. Destructive disk operations retained human authorization, automation checked safety preconditions and rate limits, and releases advanced through staged durability checks with automatic aborts. By 2024, Dropbox reported testing every invariant weekly and taking a full zone offline for about 30 minutes each week. [T2][T8][T9]

## Reported results and their limits

### Cost

Dropbox's S-1 reported a $39.5 million decrease in infrastructure costs in 2016: $92.5 million less third-party service-provider expense, partly offset by $53.0 million more depreciation, facilities, and support for its own equipment. It reported another $35.1 million decrease in 2017. These results support lower unit and infrastructure costs, but the filing's Infrastructure Optimization program also included inactive-account cleanup and usage changes. It does not disclose Magic Pocket's complete build cost, exact savings, or payback period. [F1, pp. 67, 71-73]

### Durability and availability

The 2016 design target was annual durability above 99.9999999999% and availability above 99.99%. A later model estimated 27 nines of durability. These are a target and a model, not direct measurements. The strongest migration outcome Dropbox reported was more than 90% of data served in-house without a major service disruption or data loss during the move. [B1][B3]

### Performance and efficiency

Dropbox reported median upload or download improvements of about 40% to 300% depending on user location after building the new network and storage path. That result is location-dependent and should not be read as a universal per-request improvement. [T4]

Later changes show the value of owning the stack. Removing SSD write caches raised fleet write throughput by 15% to 20%; 2024 load-test figures put per-host puts at about 1,500 per second instead of 500 and p99 write latency at about 200 ms instead of 1.2 seconds. Host-managed SMR supplied roughly 10% to 20% more capacity at little or no drive-cost difference, while cold storage reduced disk use by 25% for its target data. These are improvements from different years, not a single migration benchmark. [B5][B7][B11][T9]

## What Magic Pocket solved

Magic Pocket matched Dropbox's scale problem with a deliberately specialized system: immutable blocks and simple OSDs reduced coordination; familiar MySQL systems reduced operational unknowns; cells and zones bounded failures; replication protected new writes; erasure coding lowered steady-state storage overhead; and continuous verification made data loss detectable. Custom hardware and software then gave Dropbox the control to keep improving cost and performance. [B2][B3][B4][T6]

The result remained hybrid. Dropbox kept AWS for the remainder of user data, delivery, regional localization, and workloads whose economics or geographic reach still fit public cloud. Magic Pocket solved the large, predictable core-storage workload; it was not presented as proof that every cloud workload should move in-house. [F1, p. 120][T6][T8]
