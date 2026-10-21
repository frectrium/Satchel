# Source Log

Every source the team uses goes here, once. Reports and the Design Decision Log cite sources by their **ID** (for example `[B1]`), so an ID never changes after it is assigned.

## How to add a source

1. Take the next free ID in the right category.
2. Fill in every column. **Accessed** is the date you read it, in `YYYY-MM-DD`.
3. In **Key points**, write the facts we may use: numbers, dates, design decisions. Put long notes in a separate file under `docs/research/` and link to it.
4. Mark **Primary?** as `yes` only for material published by Dropbox or its engineers. Media coverage is always `no` and is used for context only.
5. If a page might change or vanish (pricing pages especially), save a copy on the [Wayback Machine](https://web.archive.org/save) and put that link in **Archive**.

## Categories

| Prefix | Category |
|---|---|
| `B` | Dropbox Tech Blog posts |
| `T` | Conference talks by Dropbox engineers |
| `F` | Filings (S-1, 10-K) |
| `M` | Media coverage (context only) |
| `P` | Cloud pricing pages and calculators (AWS, Azure) |
| `H` | Hardware, power, colocation and bandwidth price references |
| `O` | Other (papers, books, background reading) |

---

## B: Dropbox Tech Blog

| ID | Title | Author(s) | Date | URL | Archive | Accessed | Primary? | Key points |
|---|---|---|---|---|---|---|---|---|
| B1 | Scaling to exabytes and beyond | Akhil Gupta | 2016-03-14 | [Link](https://dropbox.tech/infrastructure/magic-pocket-infrastructure) | [Archive](https://web.archive.org/web/20260611104626/https://dropbox.tech/infrastructure/magic-pocket-infrastructure) | 2026-10-20 | yes | Announcement. Over 90% of user data in-house by 2015-10-07. 500 PB, 500M signups. Reasons: performance and unit economics. Targets: >99.9999999999% annual durability, >99.99% availability. Timeline 2013–2015. [Notes](../research/dropbox-tech-blog.md#b1-scaling-to-exabytes-and-beyond) |
| B2 | Inside the Magic Pocket | James Cowling | 2016-05-06 | [Link](https://dropbox.tech/infrastructure/inside-the-magic-pocket) | [Archive](https://web.archive.org/web/20261006133234/https://dropbox.tech/infrastructure/inside-the-magic-pocket) | 2026-10-20 | yes | Architecture reference. Immutable blocks up to 4 MB keyed by SHA-256, 1 GB buckets, volumes, zones, ~50 PB cells, MySQL Block Index and Replication Table, Master, OSDs, put/get/repair protocols. [Notes](../research/dropbox-tech-blog.md#b2-inside-the-magic-pocket) |
| B3 | Pocket watch: Verifying exabytes of data | James Cowling | 2016-07-06 | [Link](https://dropbox.tech/infrastructure/pocket-watch) | [Archive](https://web.archive.org/web/20260611101631/https://dropbox.tech/infrastructure/pocket-watch) | 2026-10-20 | yes | Reed-Solomon variant similar to LRC. Markov model gives 27 nines. Six verifiers (Disk Scrubber to Cross-zone Verifier). Verification is >50% of disk and DB load. Staging cluster release process. [Notes](../research/dropbox-tech-blog.md#b3-pocket-watch-verifying-exabytes-of-data) |
| B4 | Extending Magic Pocket Innovation with the first petabyte scale SMR drive deployment | Magic Pocket Hardware Engineering Teams | 2018-06-12 | [Link](https://dropbox.tech/infrastructure/extending-magic-pocket-innovation-with-the-first-petabyte-scale-smr-drive-deployment) | [Archive](https://web.archive.org/web/20260827200521/https://dropbox.tech/infrastructure/extending-magic-pocket-innovation-with-the-first-petabyte-scale-smr-drive-deployment) | 2026-10-20 | yes | First major tech company on SMR. Host Managed SMR, ~100 disks per 4U chassis, 1.4 PB per host, 50 Gbps NIC, SSD write cache, OSD without filesystem, Go to Rust. [Notes](../research/dropbox-tech-blog.md#b4-extending-magic-pocket-innovation-with-the-first-petabyte-scale-smr-drive-deployment) |
| B5 | How we optimized Magic Pocket for cold storage | Preslav Le | 2019-05-06 | [Link](https://dropbox.tech/infrastructure/how-we-optimized-magic-pocket-for-cold-storage) | [Archive](https://web.archive.org/web/20261006133234/https://dropbox.tech/infrastructure/how-we-optimized-magic-pocket-for-cold-storage) | 2026-10-20 | yes | Access pattern (40% of reads for data <1 day old). Replicate then erasure code. Cross-region copy doubles storage. Cold tier: fragments + XOR parity across 3 regions, 25% less disk. [Notes](../research/dropbox-tech-blog.md#b5-how-we-optimized-magic-pocket-for-cold-storage) |
| B6 | A new way of storing old files on Dropbox | Dropbox Team (company blog) | 2019-05-06 | [Link](https://blog.dropbox.com/topics/technology/a-new-way-of-storing-old-files-on-dropbox) | [Archive](https://web.archive.org/web/20260423034239/https://blog.dropbox.com/topics/technology/a-new-way-of-storing-old-files-on-dropbox) | 2026-10-20 | yes | Short announcement of B5. MP stores >90% of user data. Cold tier cuts disk usage 25% on the same hardware and network. [Notes](../research/dropbox-tech-blog.md#b6-a-new-way-of-storing-old-files-on-dropbox) |
| B7 | SMR: What we learned in our first year | Magic Pocket Hardware Engineering Teams | 2019-07-30 | [Link](https://dropbox.tech/infrastructure/smr-what-we-learned-in-our-first-year) | [Archive](https://web.archive.org/web/20260913222055/https://dropbox.tech/infrastructure/smr-what-we-learned-in-our-first-year) | 2026-10-20 | yes | ~40% of data on SMR by end of 2019. >20% savings vs previous storage design. 10–20% more data per drive at little to no extra cost. 60 to 100 disks per machine. [Notes](../research/dropbox-tech-blog.md#b7-smr-what-we-learned-in-our-first-year) |
| B8 | An update on SMR technology at Dropbox one year later | Dropbox Team (company blog) | 2019-07-30 | [Link](https://blog.dropbox.com/topics/technology/an-update-on-smr-technology-at-dropbox-one-year-later) | [Archive](https://web.archive.org/web/20260609234709/https://blog.dropbox.com/topics/technology/an-update-on-smr-technology-at-dropbox-one-year-later) | 2026-10-20 | yes | Short summary of B7 with the same figures. [Notes](../research/dropbox-tech-blog.md#b8-an-update-on-smr-technology-at-dropbox-one-year-later) |
| B9 | A look inside our sixth generation of server hardware | Eric Shobe, Jared Mednick | 2022-06-29 | [Link](https://dropbox.tech/infrastructure/sixth-generation-server-hardware) | [Archive](https://web.archive.org/web/20260827200521/https://dropbox.tech/infrastructure/sixth-generation-server-hardware) | 2026-10-20 | yes | In-house hardware since MP began in 2013. Scooby storage server: >100 drives, >2 PB per server, >20 PB per rack, 100 Gb NIC. [Notes](../research/dropbox-tech-blog.md#b9-a-look-inside-our-sixth-generation-of-server-hardware) |
| B10 | Everything in its write place: Cloud storage abstraction with Object Store | Lawrence Xing | 2022-10-11 | [Link](https://dropbox.tech/infrastructure/abstracting-cloud-storage-backends-with-object-store) | [Archive](https://web.archive.org/web/20260221112715/https://dropbox.tech/infrastructure/abstracting-cloud-storage-backends-with-object-store) | 2026-10-20 | yes | Dropbox kept S3 and HDFS for internal data after 2015. S3 request (PUT/GET) costs were the main pain. Object Store batches writes, routes to S3 or MP, saved millions a year. MP optimal object size 1–4 MB. [Notes](../research/dropbox-tech-blog.md#b10-everything-in-its-write-place-cloud-storage-abstraction-with-object-store) |
| B11 | Increasing Magic Pocket write throughput by removing our SSD cache disks | Ankur Kulshrestha, Rajat Goel, Sandeep Ummadi | 2022-12-08 | [Link](https://dropbox.tech/infrastructure/increasing-magic-pocket-write-throughput-by-removing-our-ssd-cache-disks) | [Archive](https://web.archive.org/web/20261006133234/https://dropbox.tech/infrastructure/increasing-magic-pocket-write-throughput-by-removing-our-ssd-cache-disks) | 2026-10-20 | yes | Single SSD per host capped writes. 2020 SSD wear-out incident. New inline extent format, extents 1–2 GB. Fleet write throughput +15–20%. All SSDs removed by end of Q1 2022. [Notes](../research/dropbox-tech-blog.md#b11-increasing-magic-pocket-write-throughput-by-removing-our-ssd-cache-disks) |
| B12 | After four years of SMR storage, here's what we love—and what comes next | Eric Shobe | 2023-03-08 | [Link](https://dropbox.tech/infrastructure/four-years-of-smr-storage-what-we-love-and-whats-next) | [Archive](https://web.archive.org/web/20260712215704/https://dropbox.tech/infrastructure/four-years-of-smr-storage-what-we-love-and-whats-next) | 2026-10-20 | yes | 90% of HDD fleet SMR. TB/watt 5–6x better than first 4 TB drives. 6th-gen storage needs 5 MW less and a third of the rack space per exabyte vs 4th-gen. HAMR next. [Notes](../research/dropbox-tech-blog.md#b12-after-four-years-of-smr-storage-heres-what-we-loveand-what-comes-next) |
| B13 | How the data center site selection process works at Dropbox | Edward del Rio | 2023-06-13 | [Link](https://dropbox.tech/infrastructure/how-the-data-center-site-selection-process-works-at-dropbox) | [Archive](https://web.archive.org/web/20260412215720/https://dropbox.tech/infrastructure/how-the-data-center-site-selection-process-works-at-dropbox) | 2026-10-20 | yes | Colocation cost terms: $/kW/month rent, $/kWh utility rate, escalator, PUE multiplier, incentives. Tier III facilities. No actual prices. [Notes](../research/dropbox-tech-blog.md#b13-how-the-data-center-site-selection-process-works-at-dropbox) |
| B14 | Seventh-generation server hardware at Dropbox: our most efficient and capable architecture yet | Eric Shobe, Jared Mednick | 2025-07-02 | [Link](https://dropbox.tech/infrastructure/seventh-generation-server-hardware) | [Archive](https://web.archive.org/web/20260207011032/https://dropbox.tech/infrastructure/seventh-generation-server-hardware) | 2026-10-20 | yes | Retrospective: >90% of ~600 PB moved in 2015. 40 PB (2012) to >600 PB (2016). >99% of fleet SMR. Drives up to 32 TB. 30 Gbps per PB standard. 15 kW rack budget doubled. [Notes](../research/dropbox-tech-blog.md#b14-seventh-generation-server-hardware-at-dropbox) |
| B15 | Improving storage efficiency in Magic Pocket, our immutable blob store | Facundo Agriel | 2026-04-02 | [Link](https://dropbox.tech/infrastructure/improving-storage-efficiency-in-magic-pocket-our-immutable-blob-store) | [Archive](https://web.archive.org/web/20261006133217/https://dropbox.tech/infrastructure/improving-storage-efficiency-in-magic-pocket-our-immutable-blob-store) | 2026-10-20 | yes | Trillions of blobs, millions of deletes a day. Erasure coding for nearly all data. Garbage collection then compaction (L1, L2, L3). Live Coder writes straight to EC volumes. [Notes](../research/dropbox-tech-blog.md#b15-improving-storage-efficiency-in-magic-pocket-our-immutable-blob-store) |

Full notes, with the facts we may cite and what feeds into other issues, are in [`docs/research/dropbox-tech-blog.md`](../research/dropbox-tech-blog.md).

## T: Conference talks

| ID | Title | Speaker(s) | Event, year | URL | Accessed | Primary? | Key points |
|---|---|---|---|---|---|---|---|
| T1 | Dropbox's exabyte storage system | James Cowling | Data @Scale, 2016 | [Link](https://atscaleconference.com/videos/dropboxs-exabyte-storage-system/) | 2026-10-21 | yes | Event summary for the original Magic Pocket talk. Focuses on durability, simple operation and verification; says the migration moved 2 EB of data. The public recording has no transcript or captions. [Notes](../research/conference-talks.md#t1-dropboxs-exabyte-storage-system) |
| T2 | Bridging the Safety Gap from Scripts to Full Auto-Remediation | David Mah | SREcon16 Europe, 2016 | [Link](https://www.usenix.org/conference/srecon16europe/program/presentation/mah) | 2026-10-21 | yes | How Dropbox automated risky storage maintenance with precondition checks, rate limits and staged removal of human authorization. Disk reformatting still required approval at the time of the talk. [Notes](../research/conference-talks.md#t2-bridging-the-safety-gap-from-scripts-to-full-auto-remediation) |
| T3 | Go-ing to Rust: Optimizing Storage at Dropbox | Jamie Turner | QCon San Francisco, 2016 | [Link](https://qconsf.com/sf2016/sf2016/presentation/going-rust-optimizing-storage-dropbox.html) | 2026-10-21 | yes | Event abstract on Diskotech and the choice of Rust over Go for nearly one million disks and tens of thousands of servers. No public recording was found. [Notes](../research/conference-talks.md#t3-go-ing-to-rust-optimizing-storage-at-dropbox) |
| T4 | Scaling for Exabytes: Dropbox | Jason Wilson | Network @Scale, 2017 | [Link](https://www.youtube.com/watch?v=N9f7lyMQwmQ) | 2026-10-21 | yes | Network built for the 2015 migration, its deadline and transfer scale. Reports near-700 PB scale after the move and 40% to 300% median transfer-speed gains depending on location. [Notes](../research/conference-talks.md#t4-scaling-for-exabytes-dropbox) |
| T5 | Dropbox Background on Magic Pocket | James Cowling | Storage Field Day 15, 2018 | [Link](https://techfieldday.com/video/dropbox-background-on-magic-pocket-with-james-cowling/) | 2026-10-21 | yes | Retrospective on the hybrid design, scale, validation and migration. Describes a six-month dark launch, migration above 6 PB per day and continued deliberate use of AWS. [Notes](../research/conference-talks.md#t5-dropbox-background-on-magic-pocket) |
| T6 | Dropbox Competitive Advantages from Infrastructure | James Cowling | Storage Field Day 15, 2018 | [Link](https://techfieldday.com/video/dropbox-competitive-advantages-from-infrastructure-with-james-cowling/) | 2026-10-21 | yes | Cost, performance and agility benefits of controlling infrastructure, including whole-lifecycle hardware economics and deliberate third-party cloud use. [Notes](../research/conference-talks.md#t6-dropbox-competitive-advantages-from-infrastructure) |
| T7 | The Magic Pocket: How Dropbox is handling large data sets | Said Babayev, Martin Meusburger | CS3, 2019 | [Link](https://indico.cern.ch/event/726040/contributions/3252077/) | 2026-10-21 | yes | Slide deck with 2019 scale, architecture and SMR hardware details: over 1,000 PB, more than 12,000 OSD machines and more than 650,000 disks. [Notes](../research/conference-talks.md#t7-the-magic-pocket-how-dropbox-is-handling-large-data-sets) |
| T8 | Magic Pocket: Dropbox's Exabyte-Scale Blob Storage System | Facundo Agriel | QCon San Francisco, 2022 | [Link](https://www.infoq.com/presentations/magic-pocket-dropbox/) | 2026-10-21 | yes | Current architecture and operations: three North American regions, more than 600,000 drives, replicated writes before erasure coding, cold storage and staged releases. [Notes](../research/conference-talks.md#t8-magic-pocket-dropboxs-exabyte-scale-blob-storage-system) |
| T9 | Scaling Magic Pocket: Exabyte Scale Object Storage System | Sandeep Ummadi, Eric Shobe | SNIA SDC, 2024 | [Link](https://www.snia.org/sniadeveloper/session/18461) | 2026-10-21 | yes | Updates the production design: 1 to 4 GB buckets, RS(6,3) or LRC(12,2,2), 100 to 120 PB cells, 98% SMR fleet, SSD-cache removal and weekly failure drills. [Notes](../research/conference-talks.md#t9-scaling-magic-pocket-exabyte-scale-object-storage-system) |
| T10 | Dropbox Network Infrastructure | Dzmitry Markovich | Storage Field Day 15, 2018 | [Link](https://techfieldday.com/video/dropbox-network-infrastructure-with-dzmitry-markovich/) | 2026-10-21 | yes | Companion presentation on Dropbox's edge network, including its earlier EC2-based edge and the network built to reduce user latency and packet loss. [Notes](../research/conference-talks.md#t10-dropbox-network-infrastructure) |

Full notes, including timestamped claims and updates between talks, are in [`docs/research/conference-talks.md`](../research/conference-talks.md).

## F: Filings

| ID | Document | Section / page | Date | URL | Accessed | Primary? | Key points |
|---|---|---|---|---|---|---|---|
| F1 | Dropbox, Inc. Form S-1 | pp. 66-67, 71-73, 80, 120 | 2018-02-23 | [Link](https://www.sec.gov/Archives/edgar/data/1467623/000119312518055809/d451946ds1.htm) | 2026-10-21 | yes | Infrastructure Optimization, annual cost and margin results, cash flow, capital expenditure, lease commitments and the remaining AWS role. Reports infrastructure-cost decreases of $39.5M in 2016 and $35.1M in 2017, but does not isolate Magic Pocket's total cost or savings. [Notes](../research/s1-cost-outcomes.md#f1-form-s-1-filed-2018-02-23) |
| F2 | Dropbox, Inc. 2018 Form 10-K | pp. 44-49, 57 | 2019-02-22 | [Link](https://www.sec.gov/Archives/edgar/data/1467623/000146762319000005/a12311810-k.htm) | 2026-10-21 | yes | Follow-up year: infrastructure costs decreased $23.3M through continued usage optimization, including a $16.1M depreciation reduction from longer useful-life estimates. Gross margin reached 72%; capital expenditure was $63.0M. [Notes](../research/s1-cost-outcomes.md#f2-2018-form-10-k-follow-up) |

Full notes, with reported figures separated from calculations and interpretation, are in [`docs/research/s1-cost-outcomes.md`](../research/s1-cost-outcomes.md).

## M: Media coverage

| ID | Title | Outlet | Date | URL | Accessed | Primary? | Key points |
|---|---|---|---|---|---|---|---|
| | | | | | | no | |

## P: Cloud pricing

| ID | Page / calculator | Provider | Region | URL | Archive | Accessed | Key points |
|---|---|---|---|---|---|---|---|
| | | | | | | | |

## H: Hardware and on-prem prices

| ID | Item | Vendor / source | Price | URL | Archive | Accessed | Key points |
|---|---|---|---|---|---|---|---|
| | | | | | | | |

## O: Other

| ID | Title | Author(s) | Date | URL | Accessed | Key points |
|---|---|---|---|---|---|---|
| | | | | | | |
