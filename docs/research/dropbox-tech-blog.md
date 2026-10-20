# Dropbox Tech Blog: Magic Pocket

Research notes for [#2](https://github.com/frectrium/Satchel/issues/2). Every post Dropbox has published about Magic Pocket, with the facts we may cite and the reasons Dropbox gave for its design decisions.

- **Sources:** `B1` to `B15` in the [Source Log](../sources/README.md#b-dropbox-tech-blog). All are primary sources, written by Dropbox or its engineers.
- **Accessed:** 2026-10-20. Every post was read in full, and each one has a Wayback Machine snapshot linked in the Source Log.
- **How we searched:** dropbox.tech's `Magic Pocket`, `SMR` and `HAMR` tag pages, keyword searches of dropbox.tech and blog.dropbox.com ("Magic Pocket", "exabyte", "Pocket Watch", "SMR", "erasure coding", "storage efficiency", "Diskotech"), and following the links between posts.
- Quotes are copied exactly. Anything we work out ourselves is marked *(our inference)*.

## Contents

1. [The posts at a glance](#1-the-posts-at-a-glance)
2. [Key facts by post](#2-key-facts-by-post)
3. [For #8: architecture](#3-for-8-architecture)
4. [For #9: cost, outcome and timeline](#4-for-9-cost-outcome-and-timeline)
5. [Inconsistencies between posts](#5-inconsistencies-between-posts)
6. [Gaps and leads for other issues](#6-gaps-and-leads-for-other-issues)

## 1. The posts at a glance

| ID | Date | Title | Why it matters to us |
|---|---|---|---|
| B1 | 2016-03-14 | Scaling to exabytes and beyond | The announcement: why Dropbox left S3, timeline, scale, durability target |
| B2 | 2016-05-06 | Inside the Magic Pocket | **The architecture reference.** Data model, components, put/get/repair protocols |
| B3 | 2016-07-06 | Pocket watch: Verifying exabytes of data | Erasure coding family, 27-nines model, the verifier stack Satchel copies |
| B4 | 2018-06-12 | Extending Magic Pocket Innovation with the first petabyte scale SMR drive deployment | SMR hardware, OSD redesign, Go to Rust |
| B5 | 2019-05-06 | How we optimized Magic Pocket for cold storage | Access patterns, cross-region replication cost, cold tier with 25% disk savings |
| B6 | 2019-05-06 | A new way of storing old files on Dropbox | Company-blog summary of B5 |
| B7 | 2019-07-30 | SMR: What we learned in our first year | SMR cost and energy savings, density progress |
| B8 | 2019-07-30 | An update on SMR technology at Dropbox one year later | Company-blog summary of B7 |
| B9 | 2022-06-29 | A look inside our sixth generation of server hardware | Server tiers, storage server density (Scooby) |
| B10 | 2022-10-11 | Everything in its write place: Cloud storage abstraction with Object Store | What Dropbox still keeps on S3, and the S3 request-cost problem |
| B11 | 2022-12-08 | Increasing Magic Pocket write throughput by removing our SSD cache disks | Extent format, SSD incident, latency trade-offs |
| B12 | 2023-03-08 | After four years of SMR storage, here's what we love—and what comes next | Power per TB, cost per exabyte trends, 90% SMR fleet |
| B13 | 2023-06-13 | How the data center site selection process works at Dropbox | Colocation cost terms ($/kW/month, PUE) for the on-prem model |
| B14 | 2025-07-02 | Seventh-generation server hardware at Dropbox | Data volumes 2012–2016, current drive sizes, 30 Gbps per PB standard |
| B15 | 2026-04-02 | Improving storage efficiency in Magic Pocket, our immutable blob store | Deletes, garbage collection, compaction, erasure coding for nearly all data |

## 2. Key facts by post

### B1: Scaling to exabytes and beyond

Akhil Gupta, 2016-03-14. The announcement post.

- **Result:** "we're now storing and serving over 90% of our users' data on our custom-built infrastructure", about two and a half years after the project started.
- **Scale:** over 500 million signups and 500 PB of user data. About 40 PB in 2012, "over 12x growth" in the four years since.
- **Before Magic Pocket:** a "hybrid cloud architecture", with metadata and web servers in Dropbox's own data centers and file content on Amazon S3.
- **Why build it (Dropbox's stated reasons):**
  1. **Performance**, "one of our key product differentiators": in-house storage lets Dropbox "customize the entire stack end-to-end".
  2. **Unit economics:** its block storage use case is unique, so customizing hardware and software at its scale results "in better unit economics".
- **Why build from scratch:** "there's nothing in the open source community that's proven to work reliably at our scale."
- **Targets:** data encrypted at rest. Designed for "annual data durability of over 99.9999999999%, and availability of over 99.99%".
- **Timeline:**

  | Date | Milestone |
  |---|---|
  | Before summer 2013 | Small prototype built as a proof of concept |
  | Summer 2013 | Magic Pocket becomes "a major initiative" |
  | August 2014 | "Dark launch": data mirrored between two regional locations. Extra backups kept for another 6 months |
  | 2015-02-27 | First time Dropbox stores and serves user files exclusively in-house. Goal set to scale to more than 500 PB in six months |
  | 2015-04-30 | "BASE Jump": racing to install servers in three regional locations. Peak transfer rate over half a terabit per second |
  | 2015-10-07 | 90% of data served in-house, almost a month ahead of the 2015-10-30 goal, "without any major service disruptions or any loss of data" |

- **AWS afterwards:** Dropbox will keep partnering with Amazon "where it makes sense for our users, particularly globally", and planned to store data in Germany on AWS for European business customers that request it.

### B2: Inside the Magic Pocket

James Cowling, 2016-05-06. The main architecture reference for Satchel.

**Requirements**

- **Immutable block storage:** stores "encrypted chunks of files up to 4 megabytes in size", and a block never changes once written. Mutability is handled higher up, in a separate system called FileJournal.
- **Workload:** high temporal locality. Data is read very often within an hour of upload and less after that, but every block still needs low-latency reads. Built on hard drives ("durable, cheap, storage-dense and fairly low latency"). SSDs are kept for databases and caches.
- **Durability:** "non-negotiable". Data is erasure-coded and stored across multiple geographic regions.
- **Scale:** grew "from our initial double-digit-petabyte prototypes to a multi-exabyte behemoth within the span of around 6 months".
- **Simplicity:** avoids quorum consensus and distributed coordination where possible, and uses centralized coordination instead. Chose a sharded MySQL cluster over a distributed hash table for the Block Index, which was "a really great decision in terms of simplifying development and minimizing unknowns".

**Data model**

- **Block:** an opaque chunk of a file, up to 4 MB, compressed and encrypted before reaching MP. Its key is usually the block's SHA-256 hash.
- **Bucket:** a 1 GB logical container of blocks uploaded around the same time. Reason: 4 MB is "too small a unit of granularity to move around" when replacing a disk or erasure coding data.
- **Volume:** one or more buckets stored on a set of storage nodes, either replicated or erasure coded. New blocks are replicated first. Buckets are later aggregated and erasure coded "for storage efficiency".

**Components**

| Component | Role | Notes |
|---|---|---|
| Zones | Server clusters in western, central and eastern US | Each block is stored in at least two zones, then replicated within each zone |
| Frontends | Gateway. Decide where a block goes and issue reads and writes | |
| Block Index | `hash → cell, bucket, checksum` | Sharded MySQL behind an RPC layer. Reuses Dropbox's existing MySQL expertise |
| Cross-zone replication daemon | Asynchronously copies every put to the other zone | Each block reaches the remote zone "within one second" |
| Cell | Self-contained storage cluster, around 50 PB raw | Add capacity by adding a cell. Striped across racks for physical diversity |
| OSD (storage node) | Box full of disks, over 1 PB per machine, over 8 PB per rack | "Dumb" nodes: they store blocks but don't know the topology or run distributed protocols |
| Replication Table | `bucket → volume`, `volume → OSDs, open, type, generation` | MySQL. Small enough to fit in memory |
| Master | One per cell. Watches OSDs, triggers repair, creates buckets, triggers garbage collection | Soft state, not on the data path. Limits a cell to around 100 PB |
| Volume Managers | Move and erasure-code volumes | Run on the OSD hardware to reuse idle network capacity |

**Protocols**

- **Volumes are open or closed.** Open volumes accept new writes only. Closed volumes are immutable and can be moved safely. This split lets MP use "far simpler consistency protocols".
- **Put:**
  1. The Frontend checks the Block Index for an existing copy (dedup).
  2. It picks an open volume to spread cell load and minimize cross-cluster traffic, and looks up its OSDs in the Replication Table.
  3. It writes to **all** of those OSDs, and each one fsyncs before replying.
  4. It then adds the Block Index entry.

  If an OSD fails, the Frontend retries on another volume. If the Block Index fails, it forwards the request to the other zone. Dropbox chose this over a quorum write because "Judicious management of timeouts in a retry-based scheme already results in low tail latencies".
- **Get:** Block Index, then Replication Table, then one OSD. The erasure-coding layout lets a block be read whole from a single OSD, so "most reads only hit a single disk spindle". If that OSD is down, the Frontend reconstructs the block with the Volume Manager's help.
- **Repair:**
  1. Triggered once an OSD has been offline for **15 minutes**.
  2. The Master closes that OSD's volumes and plans the reconstruction across hundreds of OSDs.
  3. It commits by bumping the volume generation number, then updating the Replication Table.
  4. A background process called Reconcile rolls back failed repairs.
- **Team:** "Most of MP was built by a team of less than half a dozen people."

### B3: Pocket watch: Verifying exabytes of data

James Cowling, 2016-07-06.

- **Erasure coding:** "a variant on Reed-Solomon erasure coding that is similar to Local Reconstruction Codes". The exact parameters are not given.
- **Durability model:** a Markov model of worst-case disk failure and repair rates gives "27 nines of durability". The post stresses this is not enough on its own: disasters, bugs and operator error matter more.
- **Defenses:** four categories, Isolation, Protection, Verification and Automation. The post covers Verification.
- **Cost of verification:** "more than 50% of the workload on our disks and databases is actually our own internal verification traffic."
- **Extent:** the physical form of a volume on a storage node, 1 GB each.
- **Repair speed:** bad disks are usually re-replicated "in less than an hour". "over half a million disks in production" in 2016.
- **The verifiers:**

  | Verifier | What it checks | How often |
  |---|---|---|
  | Disk Scrubber | Every bit on disk against checksums | Full sweep every 1–2 weeks, recent areas more often |
  | Trash Inspector | Deleted extents only held blocks that were moved or deleted | Before deletion. Trash then kept one more day |
  | Extent Referee | Every move or unlink matches an approved trash inspection | Every filesystem transition |
  | Metadata Scanner | Every Block Index entry is on the right storage nodes | About a million blocks per second, full scan about weekly |
  | Storage Watcher | Black-box fetch of 1% of written blocks after a minute, hour, day, week and month | Continuous. Written by someone outside the storage team |
  | Cross-zone Verifier | Every file in File Journal is stored in all the right zones | Continuous |

- **Release process:** integration tests with failure injection, then a Staging cluster "tens of petabytes in size" that serves live traffic. Verifiers run there for at least a week before a release is promoted.
- **Testing the verifiers:** an engineer secretly corrupted staging data. All errors were detected or repaired automatically, with no data loss.

### B4: Extending Magic Pocket Innovation with the first petabyte scale SMR drive deployment

Magic Pocket Hardware Engineering Teams, 2018-06-12.

- **SMR:** "Dropbox is the first major tech company to adopt SMR technology", adding "hundreds of petabytes of new capacity". Same 12-nines durability and 99.99% availability targets as B1.
- **Why SMR:** "greater bit density and better cost structure ($/GB), decreasing the total cost of ownership". MP's append-only, immutable 1 GB extents suit SMR's sequential-only writes.
- **Drive type:** chose Host Managed SMR, for "the most control over the way data is stored".
- **Chassis:**
  - About 100 LFF disks in a 4U chassis, 8 chassis per rack.
  - 14 TB SMR drives, so 1.4 PB per host. MP started with chassis of about 240 TB.
  - 96 GB RAM per host for the in-memory block index. 20 cores, 40 threads.
  - Over 40 Gbps write and 45 Gbps read per chassis.
  - 50 Gbps NIC, with a non-blocking Clos network and 100 Gbps uplinks.
- **Why more network bandwidth:** a bigger failure domain is acceptable "as long as the recovery time meets our SLA", so faster networking lets a chassis be drained quickly during repair.
- **Other choices:**
  - Replaced the RAID controller with an HBA, which cut provisioning time from up to 2 hours to about 30 minutes.
  - Added an SSD write cache to make disk writes non-blocking.
- **OSD design:**
  - One daemon per disk, writing to the raw block device with no filesystem.
  - Frequently changing metadata lives in the drive's conventional area. Block data lives in 256 MB sequential zones (4 × 256 MB = 1 GB).
  - Metadata is 0.03% of block data.
- **Rust:** "Moving from Go to Rust also allowed us to handle more disks, and larger disks, without increased CPU and Memory costs by being able to directly control memory allocation and garbage collection."

### B5: How we optimized Magic Pocket for cold storage

Preslav Le, 2019-05-06.

- **Access pattern:** "Over 40% of all file retrievals in Dropbox are for data uploaded in the last day, over 70% for data uploaded in the last month, and over 90% for data uploaded in the last year."
- **Encodings:** new uploads use n-way replication. Older data is re-encoded "in a more efficient erasure coded format in the background".
- **Cross-region cost:** each block is stored independently in at least two regions, in data centers "thousands of miles apart". "We effectively store double the amount of data we would otherwise."
- **Cold tier requirements:**
  - No loss of durability.
  - Must survive a full region outage plus multiple rack failures.
  - Read availability must hold. Write availability doesn't matter, because user writes go to the warm tier.
- **Rejected designs:**
  1. A single erasure code spanning all regions, killed after nine months because one software bug could wipe out everything.
  2. Facebook's Warm BLOB storage XOR-pairing approach, dropped as too complex given unpredictable deletes.
- **Chosen design:**
  - Split each block into fragments across regions, with an XOR parity fragment. For example, two data fragments plus one parity across three regions.
  - Reads ask all three regions and use the fastest two.
  - Each region keeps running independently.
- **Savings:** warm tier replication is `2 × region_internal_replication_factor`. The cold tier with three regions is `1.5 ×`, a 25% disk saving. Four regions (3+1) would give 33%.
- **Latency:** cold reads cost about one extra cross-region round trip. At p99 the cold tier was *faster* than the warm tier, because the warm tier retried naively. Dropbox then changed the warm tier to send a second request to another region if the first doesn't answer within a time budget.

### B6: A new way of storing old files on Dropbox

Dropbox Team (company blog), 2019-05-06. A short announcement of B5.

- Magic Pocket "stores and serve more than 90% of our users' data". The cold tier runs on the same hardware and network as the warm tier and reduces disk usage by 25%, "without compromising durability or availability".

### B7: SMR: What we learned in our first year

Magic Pocket Hardware Engineering Teams, 2019-07-30.

- **Rollout:** first production SMR machines arrived "in September", which is September 2018 given the post's date *(our inference)*. Since then, "all new drives for our storage servers are now SMR".
- **Density:** goal of 25% of capacity on SMR in 2019, and "close to 40 percent of Dropbox's data will be on SMR by the end of 2019".
- **Cost:**
  - SMR stores "roughly 10 to 20 percent more data" than a same-capacity PMR drive "at little to no cost difference".
  - "more than a 20% percent savings overall compared to the last generation storage design".
- **Energy:** 14 TB SMR drives replaced 8 TB drives with power draw up "only marginally". Disks per machine went from 60 to 100 on the same CPU and memory.
- **Software:**
  - The whole stack was tested on hundreds of thousands of PMR disks before SMR arrived.
  - 1 GB extents fit SMR's 256 MB zones.
  - Writes go to the disk without a filesystem, using libzbc (13 contributions upstream).
  - Built a test tool called SMRtest.
- **Test cluster:** about six racks, 48 systems, close to 5,000 drives.
- **Cold tier:** cuts disk usage by "25 to 33 percent" depending on the replication scheme.

### B8: An update on SMR technology at Dropbox one year later

Dropbox Team (company blog), 2019-07-30. A short summary of B7 with the same figures: close to 40% of data on SMR by the end of 2019, more than 20% savings over the last generation, and 10–20% more data per drive.

### B9: A look inside our sixth generation of server hardware

Eric Shobe and Jared Mednick, 2022-06-29.

- **In-house hardware:** started with Magic Pocket in 2013, to "control our own destiny in terms of cost and performance".
- **Server tiers:** Storage (runs Magic Pocket), Database, Compute and Edge. Uses a few general-purpose designs rather than one per service ("snowflakes").
- **Scooby (storage):** co-designed with the Magic Pocket team. More than 100 drives per chassis, more than 2 PB per server, more than 20 PB per rack, 100 Gb NIC, 8 chassis per rack.
- **Diego (database):** supports Edgestore, File Journal and Magic Pocket.

### B10: Everything in its write place: Cloud storage abstraction with Object Store

Lawrence Xing, 2022-10-11.

- **Still on S3:** user file data moved to MP in 2015, but Dropbox kept using S3 and HDFS for internal data such as crash traces, build artifacts, test logs and image caching.
- **S3's cost model was the pain point:** "the worst of which was the cost inefficiency of using S3's API". Many rarely read objects meant "a large PUT bill", and caches "burned pricey GET requests with each cache miss". S3 was "an expensive default choice".
- **Object Store (built 2020):**
  - Batches small PUTs into one blob.
  - Routes data to S3 or Magic Pocket.
  - Chunks large objects into 4 MB pieces for MP.
  - Deletes by crypto-shredding.
  - Saved "millions of dollars" a year.
- **MP's ideal object size:** MP "was designed to run optimally with a homogenous 1-4 MB object size".

### B11: Increasing Magic Pocket write throughput by removing our SSD cache disks

Ankur Kulshrestha, Rajat Goel and Sandeep Ummadi, 2022-12-08.

- **Problem:** each host's write throughput was capped by its single SSD.
  - A host held 100+ disks of 14–20 TB, 1.5–2 PB raw, behind one SSD.
  - A typical NVMe SSD manages 15–20 Gbps of writes.
- **Incident (early 2020):** many SSDs bought together hit their write-endurance limit together. That forced a mass repair. There was no actual durability impact.
- **Fix:**
  - Writes go straight to SMR, using a new extent format with block metadata stored inline.
  - On startup, each node rebuilds an in-memory index from its extents.
  - Extents are "typically 1-2 GB in size".
- **Results:**
  - Fleet write throughput up 15–20%. Load tests at peak showed 2–2.5x write throughput and 15–20% better p95 write latency.
  - At average load, last-mile disk write p95 rose 10–15%, which was only a 2% increase end to end.
  - All SSDs were gone by the end of Q1 2022. Previously 8–10 SSDs failed each month.

### B12: After four years of SMR storage, here's what we love—and what comes next

Eric Shobe, 2023-03-08.

- **Adoption:** 2018 first SMR. About 25% of the fleet by 2019, and "90% of our total HDD fleet size is now SMR capable" by 2023.
- **Power:** TB per watt improved about 5–6x since the first 4 TB drives. Latest drives use about 0.30 W per TB idle and 0.50 W per TB on random reads.
- **Cost per exabyte:** "Our sixth generation storage server needs five fewer megawatts and just a third of the physical rack space to serve an exabyte of data compared to our fourth generation design."
- **SMR economics:** still "roughly 10-20% more data" per drive "at little to no cost difference". Capacity grew while the number of hosts fell.
- **Next steps:**
  - Moving to libzbd and kernel zoned-device support.
  - At-scale vendor testing on 5,000+ drives.
  - HAMR for drives of 50 TB and beyond.
  - Seventh-generation design aimed at more than 6 PB per enclosure, which needed a network redesign "so we can drain and repair data at an acceptable rate".

### B13: How the data center site selection process works at Dropbox

Edward del Rio, 2023-06-13. Not about MP's design, but it lists the cost terms Dropbox negotiates for colocation, which the on-prem estimate needs.

- Describes Dropbox's storage as "a multi-metro, hybrid-cloud architecture".
- Facilities follow Uptime Institute Tier III guidelines.
- **Commercial terms:**
  - Rental rate in $/kW/month.
  - Utility rate in $/kWh.
  - Annual rent escalator.
  - Rent ramp.
  - PUE "as a cost multiplier".
  - Incentives such as rent abatement.
- **Gap:** the post gives no actual prices.

### B14: Seventh-generation server hardware at Dropbox

Eric Shobe and Jared Mednick, 2025-07-02.

- **Data volumes:** "Over 90% of the roughly 600PB of data we stored at the time was moved into data centers we managed ourselves" (2015). "We ramped from 40PB of storage in 2012 to over 600PB by 2016."
- **Fleet:** "tens of thousands of servers with millions of drives".
- **SMR:** "over 99% of our storage fleet uses shingled magnetic recording".
- **Drives:** grew from about 14 TB to over 30 TB. Adopted Western Digital's 32 TB Ultrastar HC690 SMR drive.
- **Storage throughput:** "One of our internal performance standards is 30Gbps per PB of data." Sonic (the storage tier) targets 200 Gbps per system.
- **Rack power:** a 15 kW per-rack budget, doubled by moving to four PDUs per rack.

### B15: Improving storage efficiency in Magic Pocket, our immutable blob store

Facundo Agriel, 2026-04-02.

- **Definition:** Dropbox measures storage efficiency as total disk used compared with user data actually stored.
- **Scale:** MP "stores trillions of blobs and processes millions of deletes each day".
- **Deletes:** deletes don't free space immediately. Closed volumes are never reopened.
  - Garbage collection marks unreferenced blobs.
  - Compaction then copies live blobs into new volumes and retires the old ones.
- **Erasure coding now:** "In Magic Pocket, we use erasure coding for nearly all data." It gives "the same level of fault tolerance as replication, but with significantly less additional storage".
- **Live Coder:** a newer service that writes data directly into erasure-coded volumes, skipping the replicated first write. It suits background work rather than latency-sensitive writes.
  - **Incident:** volumes it created were under-filled, in the worst cases under 5% live data. This raised storage overhead and went unnoticed for weeks.
- **Compaction strategies:**

  | Strategy | Target | How |
  |---|---|---|
  | L1 (original) | Mostly full volumes | Pack donor volumes into a highly filled host volume |
  | L2 | Moderately under-filled volumes | Dynamic-programming packing of several sparse volumes into one near-full volume. Reduced compaction overhead 2–3x faster than L1. Over a week, compaction overhead was 30–50% lower than in cells running L1 alone |
  | L3 | The sparsest volumes | Stream remaining live blobs through Live Coder into new volumes |

- **Safeguards:** compaction is rate-limited, kept local to each cell, and tuned by a dynamic control loop. Metadata load was the main constraint.

## 3. For #8: architecture

What the blog gives the architecture write-up, with the source for each point.

| Topic | What the sources say | Source |
|---|---|---|
| Block | Immutable, up to 4 MB, compressed and encrypted, keyed by SHA-256 | B2 |
| Bucket / volume / extent | 1 GB buckets. Volumes hold buckets on a set of OSDs. An extent is a volume's copy on one node, 1 GB (B3, B4) or 1–2 GB (B11) | B2, B3, B4, B11 |
| Zones and regions | At least 2 zones (B2) or regions (B5) per block, in western, central and eastern US. Async cross-zone copy within 1 second | B2, B5 |
| Cell | About 50 PB raw. One Master per cell. Practical limit about 100 PB | B2 |
| Block Index | Sharded MySQL, `hash → cell, bucket, checksum` | B2 |
| Replication Table | MySQL, `bucket → volume`, `volume → OSDs, open, type, generation` | B2 |
| Frontend | Dedup check, volume choice, write to all OSDs, retry on failure | B2 |
| OSD | One daemon per disk, raw block device, in-memory block index, Rust | B2, B4, B11 |
| Replication then EC | Replicate new data, erasure code it later in the background | B2, B5 |
| EC today | "nearly all data" erasure coded. Live Coder writes straight to EC | B15 |
| EC scheme | Reed-Solomon variant similar to Local Reconstruction Codes. **Parameters not published** | B3 |
| Repair | Starts after 15 minutes offline. Usually done in under an hour. Generation number as the commit point | B2, B3 |
| Verification | Six verifiers (table in B3 above). Over 50% of disk and DB load | B3 |
| Deletes | Garbage collection, then compaction (L1, L2, L3) | B2, B15 |
| Cold tier | Fragments plus XOR parity across 3 regions, read the fastest 2 of 3 | B5 |
| Design principle | "keep it simple": centralized coordination over consensus, MySQL over a custom key-value store | B2 |

**Open questions for Satchel's design (input to #8 and #14):**

- The EC scheme's actual data and parity counts. Dropbox never published them. We'll need a talk (#3) or pick our own and log it as a deviation.
- The replication factor inside a zone, before erasure coding.
- How long data stays replicated before it's erasure coded.
- The OSD's on-disk index format, beyond "in-memory index of blocks and their offsets/length".

## 4. For #9: cost, outcome and timeline

**Why Dropbox left S3, in its own words (B1):** performance ("customize the entire stack end-to-end") and unit economics at its scale. B1 doesn't give any dollar figures. Those should come from the S-1 (#4).

**Outcome figures the blog does give:**

| Figure | Value | Source |
|---|---|---|
| Data moved in-house | Over 90% of user data, by 2015-10-07 | B1 |
| Data volume at the time | 500 PB (B1, March 2016). "roughly 600PB" (B14, looking back at 2015) | B1, B14 |
| Growth | About 40 PB (2012) to 500 PB (2016), over 12x | B1 |
| Migration incidents | "without any major service disruptions or any loss of data" | B1 |
| Durability target | Over 99.9999999999% a year (12 nines). Modelled: 27 nines | B1, B3 |
| Availability target | Over 99.99% | B1, B4 |
| Team size | "less than half a dozen people" built most of MP | B2 |
| SMR savings | More than 20% vs the previous storage design. 10–20% more data per drive at little to no extra cost | B7, B12 |
| Cold tier savings | 25% less disk (3 regions), 33% (4 regions) | B5, B7 |
| Power | 6th-gen storage: 5 MW less and a third of the rack space per exabyte vs 4th-gen | B12 |
| Remaining AWS use | European data on AWS in Germany (planned 2016). Internal data on S3 until Object Store moved some of it to MP | B1, B10 |
| S3 cost driver | Request (PUT/GET) charges, not only storage | B10 |

**Timeline:** see the B1 table in section 2. Later milestones: SMR in 2018 (B4), cold tier in 2019 (B5), SSD cache removed by Q1 2022 (B11), about 90% SMR by 2023 (B12), over 99% SMR by 2025 (B14).

## 5. Inconsistencies between posts

| Topic | Conflict | How to handle it |
|---|---|---|
| Year SMR was adopted | B11 says "When Magic Pocket adopted SMR drives in 2017". B4 (June 2018), B7 ("A year ago", July 2019) and B12 ("In 2018") all say 2018 | Use 2018 (three sources, closest to the event). The 2017 date may refer to when design work began *(our inference)* |
| Data volume at migration | B1 (March 2016): 500 PB. B14 (2025): "roughly 600PB" in 2015, and "over 600PB by 2016" | Cite B1 for 2015–16, since it's contemporary. Mention B14's later figure as a retrospective |
| Durability | B1/B4: target over 12 nines a year. B3: a Markov model gives 27 nines | Not a contradiction: one is a design target, the other a model output. Say which one we mean |
| Extent size | 1 GB (B3, B4, B7) vs "typically 1-2 GB" (B11) | Extent size grew over time *(our inference)*. Use 1 GB for the 2016 design |
| SMR share in 2019 | B7: "close to 40 percent of Dropbox's data". B12: "SMR fleet size was around 25%" | Different measures (data vs fleet). Quote each with its own wording |
| Zone vs region | B2 says "zones", B5 says "regions", for what looks like the same idea | Treat them as the same unit *(our inference)*, but quote each post's own term |

## 6. Gaps and leads for other issues

- **No dedicated post on Diskotech.** B3 says a post on "our Diskotech architecture to support append-only SMR storage" would follow, but we couldn't find one. The SMR hardware is covered in B4 instead.
- **Talks for #3:** B3 links a talk called "Durability Theater". B4 links a presentation on how Dropbox "used Rust to optimize storage". Search results also mention an InfoQ / QCon SF 2022 talk, "Magic Pocket: Dropbox's Exabyte-Scale Blob Storage System". These may give the EC parameters.
- **For #4 (S-1):** the blog never puts a dollar figure on the savings from leaving S3. That has to come from the filing.
- **For #7 (on-prem prices):**
  - B13 lists the colocation cost terms to price.
  - B12 gives watts per TB for current drives.
  - B14 gives the 15 kW per rack budget and the 30 Gbps per PB standard.
- **Not included:** third-party write-ups (Medium, InfoQ articles, The New Stack, podcasts). They are secondary and belong under **M** if we use them.
