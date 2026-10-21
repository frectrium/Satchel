# Satchel: Project Brief

> A scaled-down clone of Dropbox's Magic Pocket, built to understand, reproduce and justify the decisions Dropbox made when it moved its storage off AWS S3, and to test whether those decisions hold up on cost.

---

## 1. Overview

This is a cloud computing case study project. Our case is **Dropbox Magic Pocket**, the in-house exabyte-scale block storage system Dropbox built to replace AWS S3 for storing user file data.

The project has two halves:

1. **Case study.** Research Magic Pocket from primary sources: why Dropbox left S3, how the system is architected, what it cost, and what the outcome was.
2. **Exercise.** Build **Satchel**, a working, smaller-scale clone of Magic Pocket's core storage design, deployed on cloud infrastructure. Every design decision in Satchel mirrors one Dropbox made publicly, and each one is justified. We then measure Satchel and model the case's workload cost on two cloud providers versus an on-premises estimate.

The theme of the exercise is **cost and cloud repatriation**: at what scale, and under what assumptions, does owning storage infrastructure beat renting it?

**Team:** 3 members, contributing equally.

---

## 2. Case Background: Dropbox Magic Pocket

Dropbox began with a hybrid architecture: its own datacenters ran metadata and web services, while Amazon S3 stored user file content. By 2016 it reported 500 million signups and 500 PB of user data, up from about 40 PB in 2012. At that scale, Dropbox said controlling the complete storage stack would improve product performance and unit economics. [B1]

The result was **Magic Pocket**, an immutable block store built on custom software and high-density hardware in leased colocation facilities. The project was not a complete exit from public cloud. Dropbox continued using AWS where geography or workload made it useful, including for part of its user data, delivery and European data-localization requirements. [B1][F1, p. 120][T5][T8]

### Migration timeline

| Date | Milestone and evidence |
|---|---|
| Before summer 2013 | A small prototype established that the storage design could work. [B1] |
| Summer 2013 | Magic Pocket became a major initiative. Dropbox said no available open-source system had been proven at its required scale. [B1] |
| August 2014 | The system dark-launched with mirrored data in two regions and extra backups retained for another six months. [B1][T5] |
| 2015-02-27 | Dropbox first stored and served selected user files exclusively on its own infrastructure. [B1][T4][T5] |
| 2015-04-30 | Base Jump began, with rapid server installation across three regions and transfer above half a terabit per second. [B1] |
| 2015-10-07 | More than 90% of user data was served in-house, ahead of the October goal and without a major service disruption or data loss during the migration. [B1] |
| Fourth quarter 2016 | The filing dates completion of the transition period in which data was duplicated between internal and third-party systems. This is later than the 90% service milestone because the measures describe different stages. [F1, p. 67] |
| 2018 | Dropbox disclosed that more than 90% of user data was on its custom infrastructure and the remainder was on AWS. It also put host-managed SMR drives into production. [F1, p. 120][B4][T9] |
| 2019 | A cold-storage layout reduced the cross-region storage overhead for cold data from 2x to 1.5x, a 25% reduction. [B5][T8] |
| 2022 | Dropbox finished removing SSD caches from storage hosts, increasing fleet write throughput by 15% to 20%. [B11] |
| 2024 | The disclosed production choices included RS(6,3) or LRC(12,2,2), 100 to 120 PB cells and an SMR fleet share of about 98%. [T9] |
| 2025-2026 | Seventh-generation hardware used drives up to 32 TB, while software work reduced space held by deleted immutable blobs and wrote new data directly into erasure-coded volumes. [B14][B15] |

### System design

Magic Pocket stores compressed and encrypted file chunks as immutable blocks of up to 4 MB, normally addressed by a SHA-256 hash. A sharded MySQL **Block Index** maps each hash to a cell and bucket; a **Replication Table** maps buckets and volumes to storage nodes. Frontends use those indexes to route puts and gets. [B2]

Storage is divided into geographic zones and independently managed cells. New blocks are synchronously replicated within one zone and asynchronously copied to another. They first land in open replicated volumes, then closed volumes are converted to erasure coding for lower storage overhead. By 2024, Dropbox publicly identified RS(6,3) and LRC(12,2,2) as production choices and described buckets of 1 to 4 GB. [B2][T9]

Each cell has simple object-storage devices, or OSDs, plus a Master and Volume Managers for repair, movement and coding. The Master is soft state and stays outside the request path. This avoids putting a distributed consensus system on every block read or write. [B2]

Durability depends on geographic redundancy, erasure coding and continuous verification rather than any single mechanism. Dropbox disclosed six classes of verifier in 2016, staged releases through durability checks, and by 2024 ran weekly tests of every invariant and automated full-zone failure drills. The published durability figures are design or modeled targets, not measurements proving that failure is impossible. [B1][B3][T8][T9]

The hardware evolved with the software. Dropbox moved its OSD implementation from Go to Rust, wrote directly to host-managed SMR disks without a conventional filesystem, later removed the SSD write cache, and continued increasing capacity per server and per rack. [B4][B9][B11][T3][T7]

### Cost and measured outcome

Dropbox's S-1 calls the migration, inactive-account cleanup and later usage work **Infrastructure Optimization**. The filing therefore supports the direction and scale of the financial improvement, but it does not isolate Magic Pocket from the other measures. [F1, p. 67]

| Reported result | 2015 | 2016 | 2017 |
|---|---:|---:|---:|
| Revenue | $603.8M | $844.8M | $1,106.8M |
| Cost of revenue | $407.4M | $390.6M | $368.9M |
| Gross margin | 33% | 54% | 67% |
| Infrastructure-cost decrease | not reported | $39.5M | $35.1M |
| Capital expenditure | $78.7M | $115.2M | $25.3M |
| Free cash flow | -$63.9M | $137.4M | $305.0M |

Source: Dropbox S-1. [F1, pp. 66-67, 71-73]

In 2016, $92.5M less third-party service-provider expense was partly offset by $53.0M more depreciation, facilities and support expense for Dropbox's own equipment, producing the reported $39.5M net infrastructure-cost decrease. In 2017, Dropbox reported a further $35.1M decrease. The two annual decreases sum to $74.6M, but that total is our calculation, not a Dropbox-reported Magic Pocket savings figure. [F1, pp. 71-73]

Gross margin doubled from 33% to 67% while free cash flow improved, but the filing also identifies revenue growth, paying-user growth, account cleanup and usage optimization as drivers. Company-wide capital expenditure and free cash flow include office and other spending, so they are not a Magic Pocket investment or payback calculation. [F1, pp. 66-67, 71-73]

The next annual filing reported a further $23.3M infrastructure-cost decrease and a 72% gross margin in 2018. That decrease included a $16.1M depreciation reduction from extending some equipment's estimated useful life, so it should not be relabeled as migration savings. [F2, pp. 45, 49]

### What the public evidence does not establish

- Dropbox has not disclosed a complete Magic Pocket build cost, migration payback period or exact AWS savings. [F1]
- The $74.6M sum is a calculation across two reported annual infrastructure-cost decreases, and the underlying optimization program was broader than the S3 migration. [F1, pp. 67, 71-73]
- “More than 90%” on custom infrastructure does not disclose the exact petabytes or spend that remained on AWS. [F1, p. 120]
- The 500 PB contemporary figure, a talk summary saying 2 EB moved, and a later retrospective of about 600 PB use different dates or measures and should not be presented as interchangeable. [B1][T1][B14]
- Public descriptions of the erasure code evolved. The 2024 parameters should not be projected backward onto the undisclosed 2016 production layout. [B3][T5][T8][T9]

### Primary sources to collect

- [x] Dropbox Tech Blog posts on Magic Pocket (announcement, architecture, hardware, operations)
- [x] Conference talks by Dropbox engineers on Magic Pocket
- [x] Dropbox S-1 filing (2018) for cost and outcome data
- [ ] Media coverage of the migration (for context only, not as a primary source)
- [ ] Current AWS S3 and Azure Blob Storage pricing pages and calculators
- [ ] Hardware, power, colocation and bandwidth price references for the on-prem estimate

---

## 3. Objectives

1. **Understand the case.** Identify the problem Dropbox faced, the solution it built, and the measured outcome.
2. **Reconstruct the architecture.** Produce a clear architecture diagram of Magic Pocket and explain each component's role.
3. **Build Satchel.** Implement Magic Pocket's core storage components at small scale and deploy them on the cloud.
4. **Justify every decision.** For each design choice in Satchel, cite the corresponding Dropbox decision and the public reasoning behind it.
5. **Measure.** Run experiments on Satchel and collect latency, scaling-behavior and cost data.
6. **Model the economics.** Use provider pricing calculators to model the workload on two cloud providers, and compare both against an on-premises estimate.

---

## 4. What Satchel Includes

Satchel reproduces Magic Pocket's **core storage path**, plus the **cloud-native pieces** needed to deploy, scale and observe it. It does not aim for exabyte scale, production-grade durability or custom hardware.

### Core storage components (from Magic Pocket)

| Satchel component | Role | Magic Pocket counterpart |
|---|---|---|
| **Frontend / API** | Accepts put, get and delete requests for blocks | Frontends |
| **Content-addressed blocks** | Immutable blocks keyed by their hash, so identical data is stored once | Block storage model |
| **Block Index** | Maps each block hash to the storage nodes holding it | Block Index |
| **Storage nodes** | Store block data on disk | OSDs |
| **Replication** | Keeps copies of each block across zones | Cross-zone replication |
| **Erasure coding** | Splits data into data and parity fragments spread across storage nodes, so it survives node loss with far less storage overhead than full copies | Erasure-coded volumes |
| **Verifier** | Background process that checks stored blocks are intact | Pocket Watch |

### Cloud-specific components

| Component | Purpose |
|---|---|
| **Multi-AZ deployment** | Map Magic Pocket's zones onto cloud availability zones |
| **Containerization** | Package each component (e.g., Docker) so it runs the same locally and in the cloud |
| **Infrastructure as code** | Reproducible cloud setup (e.g., Terraform) that can be torn down after experiments to control cost |
| **Horizontal scaling** | Add or remove storage nodes to test scaling behavior |
| **Observability** | Metrics and dashboards for latency, throughput and resource usage (e.g., Prometheus and Grafana) |

### Out of scope

- Exabyte scale or production-grade durability guarantees
- Custom hardware (modeled on paper in the on-prem estimate, not built)
- Dropbox's client sync engine and metadata systems beyond what Satchel needs

---

## 5. Test Environment

| Stage | Environment | Used for |
|---|---|---|
| **Development** | Docker Compose on team laptops, with simulated zones | Building features, functional tests |
| **Measurement** | Small cluster of AWS EC2 VMs spread across the 3 availability zones of the Mumbai region (ap-south-1) | Latency, scaling and cost experiments |
| **Baseline** | AWS S3 in the same region, hit by the same load generator | Comparing Satchel's numbers against the service Dropbox left |

- Satchel is written in **Rust**, following Dropbox's own move to Rust for parts of Magic Pocket.
- Satchel runs on the team's AWS account. Azure is used only for its pricing calculator, which needs no account or credits.
- Tear down infrastructure after each experiment run to keep costs low.
- Run the load generator from a separate VM in the same region, so network distance doesn't skew latency results.

---

## 6. The Exercise: Cost and Repatriation

The assigned exercise for this case:

> Use the provider pricing calculators to model the case's workload on two providers, then compare against an on-premises estimate.

### Cost comparison

| Option | What we model | Tooling |
|---|---|---|
| **Provider A: AWS** | S3 storage, requests, data transfer for the case workload | AWS Pricing Calculator |
| **Provider B: Azure** | Blob Storage equivalent of the same workload | Azure Pricing Calculator |
| **On-premises** | Hardware, power, space, network, staff, amortization | Our own estimate from public price references |

The workload model is derived from what Dropbox has disclosed publicly (data volume, growth, access patterns), with every assumption stated.

### Measurements on Satchel

- **Latency:** put and get latency across block sizes, including percentiles, compared with S3
- **Scaling behavior:** throughput and latency as storage nodes, data volume and concurrent clients increase
- **Replication vs. erasure coding:** storage overhead, write and read latency, and recovery behavior when a storage node fails
- **Cost:** resource usage of Satchel itself, used to sanity-check the cost model

---

## 7. Week-by-Week Plan

| Week | Tasks | Deliverables |
|---|---|---|
| **Week 1** | Find primary sources and relevant information. Extract architecture, cost and outcome data. | One-page summary of the sources found and the case study's salient points |
| **Week 2** | Case analysis: detailed study, architecture diagram, problem, solution. Finalize the exercise plan, including the erasure coding scheme and its parameters, based on what Dropbox disclosed. Create the **Team Roles doc** and the **Design Decision Log doc**. | 10-page report on the case study, exercise plan, and roles of team members |
| **Week 3** | Build Satchel's core. Run experiments and collect measurements (latency, scaling behavior, cost). Test the system and update with final results. Keep the Design Decision Log updated as decisions are made. | Add-ons to the report with results |
| **Week 4** | Report preparation and submission. Fold the Design Decision Log into the final report. | Final report |
| **Week 5** | Project viva | Viva presentation |

### Supporting documents

These live as separate docs, not in this brief.

| Document | Contents | Created by | Final by |
|---|---|---|---|
| **Team Roles** | Each member's responsibilities, with work split equally across the 3 members | End of Week 2 (goes into the Week 2 report) | End of Week 2 |
| **Design Decision Log** | For each Satchel decision: the Magic Pocket equivalent, Dropbox's stated reasoning, the source, and what Satchel simplifies | End of Week 2 | End of Week 3, before report prep begins |
