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
| | | | | | | | yes | |

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
