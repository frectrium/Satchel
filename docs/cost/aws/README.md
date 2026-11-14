# AWS Pricing Calculator estimate

Issue #23. The case workload (#15) priced on AWS S3 in Asia Pacific (Mumbai). The figures here come from the method in [docs/cost/method.md](../method.md), the same one `satchel-cost` (#52) uses.

- **Estimate link:** https://calculator.aws/#/estimate?id=25b630be142af868c490985fa4f8067879aa3664 (AWS keeps shared links for one year, so the exports are the lasting record)
- **Exports:** `aws-estimate.pdf` and `aws-estimate.csv` in this folder
- **Created:** 2026-11-12, by @JasMehta08

## How the estimate is set up

1. Open https://calculator.aws/ and choose **Create estimate**. No account is needed.
2. Make **five groups**, named `Year 1` to `Year 5`. Each group holds one year's monthly averages. The calculator shows a monthly and a 12-month total for each group.
3. In each group, add **Amazon Simple Storage Service (S3)**:
   - Region: **Asia Pacific (Mumbai)**
   - Tick **S3 Standard**, **S3 Standard - Infrequent Access** and **Data Transfer**
4. Enter that year's column from the table below. Pick **TB per month** as the storage unit. The calculator's TB is 1,024 GB, the same binary unit AWS bills in [P1], and the figures below are already converted.
5. Leave everything else at 0: S3 Select, Standard-IA PUTs, other classes, inbound transfer.
6. Save the estimate, copy the share link into this file, then export PDF and CSV into this folder.

## What to enter (per month)

| Field (per month) | Year 1 | Year 2 | Year 3 | Year 4 | Year 5 |
|---|---:|---:|---:|---:|---:|
| S3 Standard storage (TB) | 83,370.3 | 100,044.4 | 120,053.3 | 144,064.0 | 172,876.8 |
| S3 Standard: PUT, COPY, POST, LIST requests | 4,166,666,667 | 5,000,000,000 | 6,000,000,000 | 7,200,000,000 | 8,640,000,000 |
| S3 Standard: GET, SELECT and other requests | 7,500,000,000 | 9,000,000,000 | 10,800,000,000 | 12,960,000,000 | 15,552,000,000 |
| S3 Standard-IA storage (TB) | 416,851.7 | 500,222.1 | 600,266.5 | 720,319.8 | 864,383.8 |
| S3 Standard-IA: PUT, COPY, POST, LIST requests | 0 | 0 | 0 | 0 | 0 |
| S3 Standard-IA: GET, SELECT and other requests | 833,333,333 | 1,000,000,000 | 1,200,000,000 | 1,440,000,000 | 1,728,000,000 |
| S3 Standard-IA: lifecycle transitions in | 3,472,222,222 | 4,166,666,667 | 5,000,000,000 | 6,000,000,000 | 7,200,000,000 |
| S3 Standard-IA: data retrievals (TB) | 1,515.8 | 1,819.0 | 2,182.8 | 2,619.3 | 3,143.2 |
| Data transfer out to the internet (TB) | 15,158.2 | 18,189.9 | 21,827.9 | 26,193.4 | 31,432.1 |

Where the numbers come from *(our calc)*, using #15:
- **Storage:** S3 Standard holds data written in the last 12 months, 1/6 of the year's average stored. Standard-IA holds the other 5/6.
- **Requests:** all PUTs go to S3 Standard. 90% of GETs hit Standard, and 10% hit Standard-IA [B5].
- **Lifecycle transitions:** one per block moving from Standard to Standard-IA after a year.
- **Retrievals and egress:** Standard-IA retrievals are 10% of egress. Egress is twice the ingest.

## What the calculator should show

Expected cost *(our calc)* at the list prices in [P1]. If a group differs by more than 2%, note the difference and its reason below.

| Component (per month) | Year 1 | Year 2 | Year 3 | Year 4 | Year 5 |
|---|---:|---:|---:|---:|---:|
| S3 Standard storage | $1,964,102 | $2,356,809 | $2,828,059 | $3,393,558 | $4,072,156 |
| S3 Standard-IA storage | $5,890,615 | $7,068,738 | $8,482,486 | $10,178,983 | $12,214,780 |
| PUT requests | $20,833 | $25,000 | $30,000 | $36,000 | $43,200 |
| Standard GET requests | $3,000 | $3,600 | $4,320 | $5,184 | $6,221 |
| Standard-IA GET requests | $833 | $1,000 | $1,200 | $1,440 | $1,728 |
| Standard-IA retrievals | $15,522 | $18,626 | $22,352 | $26,822 | $32,187 |
| Lifecycle transitions | $34,722 | $41,667 | $50,000 | $60,000 | $72,000 |
| Internet egress | $1,242,462 | $1,490,815 | $1,788,838 | $2,146,466 | $2,575,619 |
| **Total per month** | **$9,172,090** | **$11,006,256** | **$13,207,254** | **$15,848,453** | **$19,017,891** |
| **Total per year** | **$110,065,080** | **$132,075,067** | **$158,487,052** | **$190,181,434** | **$228,214,692** |

Five-year total: **$819,023,325**.

## Differences from the expected figures

Every year is within 0.0001% of the expected total. Each group comes out $10.93 a month higher, because the calculator leaves out the free 100 GB of egress [P1]: 100 GB at $0.1093.

| Per month | Year 1 | Year 2 | Year 3 | Year 4 | Year 5 |
|---|---:|---:|---:|---:|---:|
| Expected | $9,172,090 | $11,006,256 | $13,207,254 | $15,848,453 | $19,017,891 |
| Calculator | $9,172,100.91 | $11,006,266.48 | $13,207,265.28 | $15,848,463.74 | $19,017,901.93 |

Over 5 years the calculator shows $819,023,980.08 against the expected $819,023,325. Its "Total 12 months cost" adds up every group, and each group holds one year's monthly average, so here it is the 5-year total.

- **Egress above 500 TB a month.** The calculator took every year's figure (15,522,043 to 32,186,508 GB a month) without a cap or a warning, and priced everything above 150 TB at $0.08 per GB. AWS still says to contact sales above 500 TB a month [P1], so this is a list-price upper bound.
- **The free 100 GB of egress.** Not applied by the calculator in any group, as noted above.
- **Units.** Storage, retrievals and egress were entered in GB per month rather than TB: the GB figures are the TB figures above times 1,024, without rounding to one decimal. The egress field takes whole numbers only.
- **Groups.** The five year groups sit inside the calculator's default top-level group, "My service group", which therefore shows the whole estimate.

## Sensitivity cases

Run as separate saved estimates, or leave to `satchel-cost` and note that here:

| Case | Change from the base |
|---|---|
| Growth -50% | 10% a year instead of 20% |
| Growth +50% | 30% a year |
| Egress -50% | 1x ingest instead of 2x |
| Egress +50% | 3x ingest |

`satchel-cost` (#52) produces all four, so the calculator only needs the base case.
