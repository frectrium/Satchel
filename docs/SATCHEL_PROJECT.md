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

*To be expanded and verified against primary sources in Week 1.*

- Dropbox originally stored user file contents on AWS S3, while keeping metadata in its own data centers.
- In 2016 Dropbox announced it had moved the large majority of user data off S3 onto Magic Pocket, its own storage system running in its own data centers.
- Dropbox later reported significant infrastructure cost savings in the years after the migration (see its 2018 S-1 filing).
- Key architectural ideas to study include immutable content-addressed blocks, a block index, zones and cells, storage nodes (OSDs), replication and erasure coding, background verification, and custom high-density storage hardware.

### Primary sources to collect

- [ ] Dropbox Tech Blog posts on Magic Pocket (announcement, architecture, hardware, operations)
- [ ] Conference talks by Dropbox engineers on Magic Pocket
- [ ] Dropbox S-1 filing (2018) for cost and outcome data
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
