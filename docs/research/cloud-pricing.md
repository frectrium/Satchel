# Cloud pricing: AWS S3 (ap-south-1) and Azure Blob Storage (Central India)

Issue #6. Collected 2026-10-20. List prices in USD, excluding taxes [P1]. Prices change, so every figure here is tied to a Source Log ID and an access date. Re-check before the Week 3 cost runs (#23, #24), which use the AWS Pricing Calculator [P3] and the Azure Pricing Calculator [P7].

Our measurement region is AWS **ap-south-1 (Mumbai)**. The Azure counterpart is **Central India**.

AWS bills storage in binary units: 1 GB is 2^30 bytes and 1 TB is 1,024 GB [P1]. Azure volume bands are listed in GB in the price API [P5].

## 1. AWS S3, Asia Pacific (Mumbai)

### 1.1 Storage, per GB-month [P1]

| Storage class | Band | Price |
|---|---|---|
| S3 Standard | First 50 TB | $0.025 |
| S3 Standard | Next 450 TB | $0.024 |
| S3 Standard | Over 500 TB | $0.023 |
| S3 Intelligent-Tiering, Frequent Access | First 50 TB / next 450 TB / over 500 TB | $0.025 / $0.024 / $0.023 |
| S3 Intelligent-Tiering, Infrequent Access | All | $0.0138 |
| S3 Intelligent-Tiering, Archive Instant Access | All | $0.005 |
| S3 Intelligent-Tiering, Archive Access (optional) | All | $0.0045 |
| S3 Intelligent-Tiering, Deep Archive Access (optional) | All | $0.002 |
| S3 Intelligent-Tiering monitoring and automation | Objects over 128 KB | $0.0025 per 1,000 objects |
| S3 Standard-IA | All | $0.0138 |
| S3 One Zone-IA | All | $0.011 |
| S3 Express One Zone | All | $0.124 |
| S3 Glacier Instant Retrieval | All | $0.005 |
| S3 Glacier Flexible Retrieval | All | $0.0045 |
| S3 Glacier Deep Archive | All | $0.002 |

### 1.2 Requests, per 1,000 requests [P1]

| Storage class | PUT, COPY, POST, LIST | GET, SELECT and other | Lifecycle transition into |
|---|---|---|---|
| S3 Standard | $0.005 | $0.0004 | $0.00 |
| S3 Standard-IA | $0.01 | $0.001 | $0.01 |
| S3 One Zone-IA | $0.01 | $0.001 | $0.01 |
| S3 Express One Zone | $0.00113 | $0.00003 | n/a |
| S3 Glacier Instant Retrieval | $0.02 | $0.01 | $0.02 |
| S3 Glacier Flexible Retrieval | $0.036 | $0.0004 | $0.036 |
| S3 Glacier Deep Archive | $0.07 | $0.0004 | $0.07 |

DELETE and CANCEL requests are free. LIST requests for any class are charged at the S3 Standard PUT rate [P1].

### 1.3 Data retrieval [P1]

| Storage class / option | Per 1,000 retrieval requests | Per GB retrieved |
|---|---|---|
| S3 Standard | $0.00 | $0.00 |
| S3 Standard-IA | n/a | $0.01 |
| S3 One Zone-IA | n/a | $0.01 |
| S3 Glacier Instant Retrieval | n/a | $0.03 |
| S3 Glacier Flexible Retrieval, Expedited | $12.00 | $0.036 |
| S3 Glacier Flexible Retrieval, Standard | $0.06 | $0.012 |
| S3 Glacier Flexible Retrieval, Provisioned Capacity Unit | n/a | $120.00 per unit |
| S3 Glacier Deep Archive, Standard | $0.12 | $0.024 |
| S3 Glacier Deep Archive, Bulk | $0.03 | $0.005 |
| S3 Intelligent-Tiering, Archive Access, Expedited | $12.00 | $0.036 |

### 1.4 Data transfer [P1]

| Direction | Band | Price per GB |
|---|---|---|
| In from the internet | All | $0.00 |
| Out to the internet | First 10 TB / month | $0.1093 |
| Out to the internet | Next 40 TB / month | $0.085 |
| Out to the internet | Next 100 TB / month | $0.082 |
| Out to the internet | Over 150 TB / month | $0.08 |
| Out to Amazon CloudFront | All | $0.00 |
| Out to another AWS region (any listed region) | All | $0.086 |

- The first 100 GB of internet egress each month is free, aggregated across all AWS services and regions [P1].
- Above 500 TB a month, AWS says to contact them for pricing [P1].
- Transfer between S3 buckets, or from S3 to another AWS service, in the same region is free [P1].

### 1.5 Minimum durations, minimum sizes and other fees [P1]

| Class | Minimum billable object size | Minimum storage duration | Other |
|---|---|---|---|
| S3 Standard | none | none | |
| S3 Standard-IA, S3 One Zone-IA | 128 KB | 30 days | Per-GB retrieval fee |
| S3 Glacier Instant Retrieval | 128 KB | 90 days | Per-GB retrieval fee |
| S3 Glacier Flexible Retrieval | none stated | 90 days | 40 KB of metadata per archived object (8 KB at Standard rates, 32 KB at Glacier rates) |
| S3 Glacier Deep Archive | none stated | 180 days | Same 40 KB metadata charge |
| S3 Intelligent-Tiering | Objects under 128 KB are never auto-tiered and always pay Frequent Access rates | none stated | No retrieval fees; monitoring fee per object |

Objects deleted, overwritten or transitioned before the minimum duration pay a pro-rated charge for the remaining days [P1].

### 1.6 Availability zones, durability and access time [P2]

| Class | AZs | Durability | First-byte latency |
|---|---|---|---|
| S3 Standard | at least 3 | 11 nines | milliseconds |
| S3 Intelligent-Tiering | at least 3 | 11 nines | milliseconds (frequent and infrequent tiers) |
| S3 Standard-IA | at least 3 | 11 nines | milliseconds |
| S3 One Zone-IA | 1 | 11 nines, but can lose data if the AZ is lost | milliseconds |
| S3 Glacier Instant Retrieval | at least 3 | 11 nines | milliseconds |
| S3 Glacier Flexible Retrieval | multiple | 11 nines | minutes or hours |
| S3 Glacier Deep Archive | at least 3 | 11 nines | hours |

## 2. Azure Blob Storage, Central India

All figures are from the Azure Retail Prices API for General Block Blob v2 (the general-purpose v2 storage account) and the Bandwidth service in `centralindia` [P5, P6]. The public pricing page [P4] and the calculator [P7] show the same data.

### 2.1 Storage, per GB-month [P5]

| Tier | LRS | ZRS | GRS |
|---|---|---|---|
| Hot, first 50 TB (from 0 GB) | $0.02 | $0.025 | $0.04 |
| Hot, next 450 TB (from 51,200 GB) | $0.0192 | $0.024 | $0.0384 |
| Hot, over 500 TB (from 512,000 GB) | $0.0184 | $0.023 | $0.0368 |
| Cool | $0.011 | $0.0138 | $0.026 |
| Cold | $0.0045 | $0.005 | $0.00994 |
| Archive | $0.002 | not offered | $0.005 |

The archive tier is only available on LRS, GRS and RA-GRS accounts, not ZRS [P8].

### 2.2 Operations, per 10,000 operations [P5]

| Tier and redundancy | Write | Read | List and create container | All other |
|---|---|---|---|---|
| Hot LRS | $0.055 | $0.0044 | $0.055 | $0.0044 |
| Hot ZRS | $0.06875 | $0.0044 | $0.06875 | $0.0044 |
| Hot GRS | $0.11 | $0.0044 | $0.11 | $0.0044 |
| Cool LRS | $0.1 | $0.01 | | |
| Cool ZRS | $0.1 | $0.01 | | |
| Cool GRS | $0.2 | $0.01 | | |
| Cold LRS | $0.18 | $0.1 | | |
| Cold ZRS | $0.2 | $0.1 | | |
| Cold GRS | $0.452 | $0.1 | | |
| Archive LRS | $0.12 | $6.00 | | |
| Archive GRS | $0.27 | $6.00 | | |

Blank cells: the API returned no separate meter for that combination [P5].

### 2.3 Data retrieval, write and early deletion, per GB [P5]

| Tier | Data retrieval | Data write | Early deletion (LRS / ZRS) |
|---|---|---|---|
| Hot | none listed | none listed | none |
| Cool | $0.01 | $0.00 | $0.0152 / $0.019 |
| Cold | $0.03 | $0.00 | $0.0045 / $0.005 |
| Archive, standard priority | $0.024 | $0.00 | $0.002 / not offered |
| Archive, high priority | $0.14 (plus $70.00 per 10,000 reads) | | |

### 2.4 Minimum durations [P8]

| Tier | Minimum storage duration | Notes |
|---|---|---|
| Hot | none | |
| Cool | 30 days | |
| Cold | 90 days | |
| Archive | 180 days | Offline tier. Rehydration can take up to 15 hours |

[P8] does not state a minimum billable object size for Azure.

### 2.5 Data transfer [P6]

The API lists thresholds in GB per month. The TB labels are our reading of those thresholds *(our inference)*.

| Direction | Band (API threshold in GB) | Microsoft network routing (default) | Internet routing |
|---|---|---|---|
| In from the internet | All | $0.00 | |
| Out to the internet | First 100 GB | $0.00 | $0.00 |
| Out to the internet | From 100 GB (about the next 10 TB) | $0.12 | $0.11 |
| Out to the internet | From 10,335 GB / 10,100 GB (about the next 40 TB) | $0.085 | $0.075 |
| Out to the internet | From 51,295 GB / 50,100 GB (about the next 100 TB) | $0.082 | $0.07 |
| Out to the internet | From 153,695 GB / 150,100 GB (over about 150 TB) | $0.08 | $0.06 |
| Between availability zones | All, each direction | $0.01 | |
| Between regions | All | $0.08 | |

### 2.6 Redundancy [P9]

- **LRS** keeps all copies in one datacenter in the region. At least 11 nines durability a year.
- **ZRS** spreads data across three or more availability zones in the region. At least 12 nines durability a year.

## 3. Like-for-like comparison

S3 Standard keeps data in at least 3 AZs [P2]. On Azure, the matching option is **ZRS** [P9], not the cheaper LRS, which stays in a single datacenter *(our inference)*. Per-request prices below are converted to per 1,000 so they line up *(our arithmetic)*.

| Item | AWS S3 Standard, ap-south-1 | Azure Hot ZRS, Central India |
|---|---|---|
| Storage, first 50 TB | $0.025 per GB-month [P1] | $0.025 per GB-month [P5] |
| Storage, over 500 TB | $0.023 per GB-month [P1] | $0.023 per GB-month [P5] |
| Writes, per 1,000 | $0.005 [P1] | $0.006875 [P5] |
| Reads, per 1,000 | $0.0004 [P1] | $0.00044 [P5] |
| Internet egress, first band | $0.1093 per GB [P1] | $0.12 per GB [P6] |
| Internet egress, top band | $0.08 per GB above 150 TB [P1] | $0.08 per GB above about 150 TB [P6] |

Storage list prices are the same at these volumes. The differences show up in requests and egress.

## 4. Which tiers fit Dropbox's workload (first guess)

Everything in this section is *(our inference)* and gets refined in Week 2 (#15) once the architecture (#8) and S-1 (#4) numbers are in.

- **Assumed workload:** most stored data is cold, but users expect any file to open right away, and recent files are read often.
- **Recent, frequently read data:** S3 Standard [P1] or Azure Hot ZRS [P5].
- **Older data that still needs instant access:** S3 Standard-IA or S3 Glacier Instant Retrieval [P1], or Azure Cool or Cold ZRS [P5]. All of these keep millisecond access [P2, P8] but add per-GB retrieval fees and minimum storage durations.
- **Not a fit:** S3 Glacier Flexible Retrieval, Deep Archive and Azure Archive. Restores take minutes to hours [P2, P8], which a user waiting on a file won't accept.
- **S3 Intelligent-Tiering** [P1] is a reasonable single-class option for the "unknown access pattern" case. It has no retrieval fees but charges a monitoring fee per object.
- **Object size matters.** Classes with a 128 KB minimum billable size [P1] penalise small objects. Whether that hurts depends on the block size Magic Pocket uses, which #8 should confirm from a primary source.

## 5. Uncertainties and open questions

- **Region-specific values are not in the archived S3 page.** The S3 pricing page loads prices by script. The Wayback copy [P1] may only show the default region (US East). The Mumbai values above were read from the live page on 2026-10-20 with the region set to Asia Pacific (Mumbai).
- **Exabyte scale is off the published price lists.** The largest published band is "over 500 TB" for storage, and AWS asks customers moving more than 500 TB a month to contact them [P1]. Dropbox's scale, and any discount it negotiated, is far beyond that, so list prices are an upper bound *(our inference)*.
- **Azure egress bands.** The Retail Prices API gives raw GB thresholds. The pricing page [P4] should be checked to confirm the TB labels in 2.5.
- **Azure inter-AZ transfer.** The API lists $0.01 per GB between availability zones [P6]. Check whether this applies to ZRS replication traffic. We assume it does not, since ZRS replication is part of the storage price *(our inference)*.
- **Currency and billing entity.** All prices are USD list prices excluding taxes. Indian accounts may be billed in INR through a local entity. This needs checking before the cost model is final.
- **Reserved capacity.** Azure offers reserved capacity for Blob Storage (a separate meter in the API). It is not included here, so the Azure figures are pay-as-you-go only.
