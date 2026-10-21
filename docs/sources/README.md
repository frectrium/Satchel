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
| | | | | | | yes | |

## F: Filings

| ID | Document | Section / page | Date | URL | Accessed | Primary? | Key points |
|---|---|---|---|---|---|---|---|
| | | | | | | yes | |

## M: Media coverage

| ID | Title | Outlet | Date | URL | Accessed | Primary? | Key points |
|---|---|---|---|---|---|---|---|
| | | | | | | no | |

## P: Cloud pricing

| ID | Page / calculator | Provider | Region | URL | Archive | Accessed | Key points |
|---|---|---|---|---|---|---|---|
| P1 | Amazon S3 pricing (Storage & requests, Data transfer tabs, region set to Asia Pacific (Mumbai)) | AWS | ap-south-1 | https://aws.amazon.com/s3/pricing/ | https://web.archive.org/web/20261020162106/https://aws.amazon.com/s3/pricing/ | 2026-10-20 | S3 Standard $0.025 / $0.024 / $0.023 per GB-month (first 50 TB / next 450 TB / over 500 TB). PUT $0.005, GET $0.0004 per 1,000. Internet egress $0.1093 per GB for the first 10 TB, down to $0.08 above 150 TB. Full tables in `docs/research/cloud-pricing.md`. Prices load by script, so the archived copy may show the default region only |
| P2 | Amazon S3 storage classes | AWS | All regions | https://aws.amazon.com/s3/storage-classes/ | https://web.archive.org/web/20261021045450/https://aws.amazon.com/s3/storage-classes/ | 2026-10-20 | S3 Standard, Standard-IA, Glacier Instant Retrieval and Intelligent-Tiering store data in at least 3 AZs; One Zone-IA in 1 AZ. 11 nines durability. Millisecond access except Glacier Flexible Retrieval (minutes or hours) and Deep Archive (hours) |
| P3 | AWS Pricing Calculator | AWS | ap-south-1 | https://calculator.aws/ | n/a (interactive tool) | 2026-10-20 | Tool for #23. Its public S3 price file (`calculator.aws/pricing/2.0/meteredUnitMaps/s3/USD/current/s3.json`) reports publication date 2026-10-15T17:35:40Z |
| P4 | Azure Blob Storage pricing | Microsoft Azure | Central India | https://azure.microsoft.com/en-us/pricing/details/storage/blobs/ | https://web.archive.org/web/20261021045536/https://azure.microsoft.com/en-us/pricing/details/storage/blobs/ | 2026-10-20 | Human-readable version of the prices in P5. Figures in our notes are taken from P5 |
| P5 | Azure Retail Prices API: Storage, General Block Blob v2, Central India | Microsoft Azure | Central India | https://prices.azure.com/api/retail/prices?$filter=serviceName%20eq%20%27Storage%27%20and%20armRegionName%20eq%20%27centralindia%27%20and%20productName%20eq%20%27General%20Block%20Blob%20v2%27 | https://web.archive.org/web/20261021045621/https://prices.azure.com/api/retail/prices?$filter=serviceName%20eq%20%27Storage%27%20and%20armRegionName%20eq%20%27centralindia%27%20and%20productName%20eq%20%27General%20Block%20Blob%20v2%27 | 2026-10-20 | Hot ZRS $0.025 / $0.024 / $0.023 per GB-month (bands start at 0, 51,200 and 512,000 GB). Hot LRS $0.02 / $0.0192 / $0.0184. Cool LRS $0.011, Cold LRS $0.0045, Archive LRS $0.002. Operations priced per 10,000 |
| P6 | Azure Retail Prices API: Bandwidth, Central India | Microsoft Azure | Central India | https://prices.azure.com/api/retail/prices?$filter=serviceName%20eq%20%27Bandwidth%27%20and%20armRegionName%20eq%20%27centralindia%27 | https://web.archive.org/web/20261021045731/https://prices.azure.com/api/retail/prices?$filter=serviceName%20eq%20%27Bandwidth%27%20and%20armRegionName%20eq%20%27centralindia%27 | 2026-10-20 | Internet egress (Microsoft network routing): first 100 GB free, then $0.12, $0.085, $0.082, $0.08 per GB by volume band. Data in free. Inter-AZ transfer $0.01 per GB each way |
| P7 | Azure Pricing Calculator | Microsoft Azure | Central India | https://azure.microsoft.com/en-us/pricing/calculator/ | n/a (interactive tool) | 2026-10-20 | Tool for #24. No account needed |
| P8 | Access tiers for blob data (Microsoft Learn) | Microsoft Azure | All regions | https://learn.microsoft.com/en-us/azure/storage/blobs/access-tiers-overview | https://web.archive.org/web/20261021045752/https://learn.microsoft.com/en-us/azure/storage/blobs/access-tiers-overview | 2026-10-20 | Minimum storage: Cool 30 days, Cold 90 days, Archive 180 days. Archive rehydration can take up to 15 hours. Archive tier only on LRS, GRS and RA-GRS, not ZRS. Page last updated 2026-06-15 |
| P9 | Azure Storage redundancy (Microsoft Learn) | Microsoft Azure | All regions | https://learn.microsoft.com/en-us/azure/storage/common/storage-redundancy | https://web.archive.org/web/20261021045829/https://learn.microsoft.com/en-us/azure/storage/common/storage-redundancy | 2026-10-20 | LRS: single datacenter, at least 11 nines durability. ZRS: three or more availability zones, at least 12 nines. Page last updated 2026-08-11 |

## H: Hardware and on-prem prices

| ID | Item | Vendor / source | Price | URL | Archive | Accessed | Key points |
|---|---|---|---|---|---|---|---|
| | | | | | | | |

## O: Other

| ID | Title | Author(s) | Date | URL | Accessed | Key points |
|---|---|---|---|---|---|---|
| | | | | | | |
