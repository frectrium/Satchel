# Week 1 summary: sources and salient points

**Satchel**, a scaled-down clone of Dropbox Magic Pocket. Team: Parshv Joshi (@frectrium), @VyasGuru, @JasMehta08. Source IDs refer to the [Source Log](../sources/README.md).

## Sources found

| Type | Count | Most important |
|---|---:|---|
| Dropbox Tech Blog (B, primary) | 15 | *Inside the Magic Pocket* [B2], *Scaling to exabytes and beyond* [B1], *Pocket watch* [B3] |
| Conference talks (T, primary) | 10 | QCon SF 2022 [T8] and SNIA SDC 2024 [T9] for the current design, Storage Field Day 2018 [T5] for the migration |
| SEC filings (F, primary) | 2 | 2018 S-1 [F1], 2018 10-K [F2] |
| Media (M, context only) | 8 | GeekWire [M5] and Data Center Knowledge [M6] on the S-1 savings |
| Cloud pricing (P) | 9 | S3 pricing, ap-south-1 [P1]. Azure Retail Prices API, Central India [P5] |
| On-prem prices (H) | 13 | HDD $/TB [H2], PUE [H6], Indian colocation [H8], IP transit [H9] |

## Problem: why Dropbox left S3

By 2016 Dropbox held 500 PB of user data, up more than 12x from about 40 PB in 2012 [B1]. File content lived on S3 while metadata ran in Dropbox's own data centers [B1]. Dropbox gave two reasons for building its own storage: **performance**, by controlling the stack end to end, and **better unit economics** at its scale [B1]. In a later talk, cost was described as the leading reason [T6]. An expiring AWS storage commitment set the migration deadline [T4].

## Solution: Magic Pocket

- **Data model:** an immutable block store. Blocks are up to 4 MB and keyed by SHA-256. Each block is stored in at least two US zones and replicated within each zone [B2].
- **Indexes:** a sharded MySQL Block Index maps each block to a cell and bucket [B2].
- **Cells:** each cell has storage nodes (OSDs), plus a Master and Volume Managers that handle repair and erasure coding outside the request path [B2].
- **Encoding:** new data is replicated (4 copies per zone), then erasure coded. The 2024 production schemes are RS(6,3) and LRC(12,2,2) [T8][T9].
- **Verification:** six continuous verifiers check durability and use over half the disk and database load [B3].
- **Hardware:** dense SMR servers. Storage nodes were rewritten from Go to Rust for memory control [B4].

Full notes: [architecture.md](../research/architecture.md).

## Outcome

- **Migration:** over 90% of user data was served in-house by 2015-10-07, "without any major service disruptions or any loss of data" [B1]. The duplicated copies were retired in Q4 2016 [F1].
- **Cost:** infrastructure costs fell **$39.5M** in 2016 and **$35.1M** in 2017. Gross margin rose from 33% (2015) to 67% (2017) [F1]. The S-1 groups the migration with account cleanup under "Infrastructure Optimization", so these figures aren't Magic Pocket savings alone [F1].
- **Hybrid result:** AWS still serves the remaining data, delivery and European data residency [F1][T8].
- **Later savings:**
  - SMR drives: more than 20% vs the previous storage design [B7].
  - Cold tier: 25% less disk [B5].

## Inputs for the cost exercise so far

**Cloud, list prices:**

| | Storage, per GB-month | Writes, per 1,000 | Egress |
|---|---|---|---|
| S3 Standard, ap-south-1 [P1] | $0.025, falling to $0.023 | $0.005 | $0.1093/GB, falling to $0.08/GB |
| Azure Hot ZRS, Central India [P5][P6] | Same as S3 | $0.006875 | $0.12/GB, falling to $0.08/GB |

**On-prem:**

| Item | Value | Source |
|---|---|---|
| Enterprise HDD | $27.92–44.17/TB (Aug 2026), up 15–48% since March | [H2] |
| PUE, Asia-Pacific | 1.52 | [H6] |
| Indian colocation | Rs 25,000–60,000 per rack per month, plus power per kVA | [H8] |
| IP transit | From $0.03/Mbps/month | [H9] |
| Server life | 5–6 years | [H13] |

## Open questions for Week 2

1. How long does Dropbox keep data replicated before erasure coding it? No source says. This decides Satchel's parameters (#14).
2. Which workload size do we model: 500 PB [B1] or about 600 PB [B14]? And for which year (#15)?
3. How do we turn the 2019 full-server cost [H3] into a 2026 figure, given volatile drive prices [H2]?
4. Which S3 and Azure tiers match Dropbox's mostly-cold, instant-read workload (#15)?
