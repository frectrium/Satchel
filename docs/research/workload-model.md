# Workload model

Issue #15. The single workload we price on AWS (#23), Azure (#24) and on-prem (#25), and compare in #26.

- Every parameter has a source ID or is labelled as an assumption. Numbers we calculate from sources are marked *(our calc)*.
- Units are decimal: 1 PB = 1,000 TB = 10^6 GB. AWS bills binary GB (2^30 bytes) [P1], so #23 multiplies GB figures here by 0.9313.

## 1. Scope decisions

These three choices were made by the issue owner because no source settles them. Reviewers should confirm or change them in this PR.

| Decision | Choice | Why |
|---|---|---|
| Baseline | Dropbox at the end of the migration: **2016, 500 PB of user data** | The moment Dropbox chose to own storage [B1]. Matches the cost years in the S-1 [F1] |
| Horizon | **5 years** (years 1 to 5 after the baseline) | Hyperscale server life is 5 to 6 years [H13]. Dropbox depreciated over 3, then 4 years [F2]. 5 years lets owned hardware pay back without assuming a second refresh |
| Scale | **Base case at 500 PB, plus a sweep** at 0.1, 1, 10, 100 and 500 PB | The exercise asks at what scale owning beats renting. The sweep keeps every ratio the same and only changes the starting size |

All years are priced at **2026 prices** (the P and H sources). We price a 2016-sized workload with today's prices because 2016 price lists were not collected *(our inference)*.

## 2. Parameters

| # | Parameter | Value | Source or assumption |
|---|---|---|---|
| 1 | Data stored at baseline | 500 PB of user data | [B1]. Dropbox later wrote "roughly 600PB" for the same period [B14]; see section 7 |
| 2 | Growth, base case | 20% a year | "over 20% annual data growth" in 2018 [T5] |
| 3 | Growth, high case | 41% a year | *(our calc)* from 500 PB in 2016 [B1] to about 1 EB in 2018 [T5]. 2012 to 2016 was 88% a year [B1] *(our calc)*, too early to project |
| 4 | Block size | Up to 4 MB, immutable, keyed by SHA-256 | [B2] |
| 5 | Average block size | **2 MB** (sensitivity 1 MB and 4 MB) | Assumption. No size distribution is published. Magic Pocket "was designed to run optimally with a homogenous 1-4 MB object size" [B10]. 2 MB is the midpoint |
| 6 | Objects stored at baseline | 250 billion blocks | *(our calc)* 500 PB / 2 MB. Same order as "trillions of blobs" in 2026 [B15] |
| 7 | Ingest (new data written) | Equal to net growth: 100 PB in year 1 | Assumption: no deletes, so every byte of growth is a new write. Deletes are not quantified for 2016 |
| 8 | Write requests (PUT) | Ingest / average block size: 50 billion in year 1, about 1,600 a second | *(our calc)* from rows 5 and 7 |
| 9 | Egress (data served to users) | **2 x ingest**: 200 PB in year 1 (sensitivity 1x and 5x) | Assumption. Dropbox syncs each upload to a user's other devices and to people it is shared with, so downloads track uploads. Reads concentrate on new data [B5] |
| 10 | Read requests (GET) | Egress / average block size: 100 billion in year 1, about 3,200 a second | *(our calc)* from rows 5 and 9 |
| 11 | Read requests, upper bound | About 62,500 a second at 500 PB | *(our calc)*: Dropbox reported over 2 Tb/s of traffic at about 1 EB in 2018 [T5], so 1 Tb/s at 500 PB, in 2 MB requests. This includes internal traffic, which verification alone makes over half of disk load [B3], so it overstates client reads |
| 12 | Which data is read | Over 40% of retrievals are of data uploaded in the last day, over 70% in the last month, over 90% in the last year | [B5] |
| 13 | Hot vs cold split | Hot = data written in the last 12 months: 16.7% of stored data at 20% growth. Cold = the rest, 83.3% | *(our calc)* from row 2. The 12-month line follows the 90%-in-a-year figure in row 12 |
| 14 | Reads that hit cold data | 10% of egress | From row 12: under 10% of retrievals are of data older than a year [B5] |
| 15 | Data lifetime | Kept for the whole horizon, no deletes | Assumption. Blocks are immutable [B2]. Dropbox does delete ("millions of deletes each day" in 2026 [B15]) and cleaned up inactive accounts [F1], but neither is quantified for 2016. Keeping everything overstates storage slightly |
| 16 | Durability target | Over 99.9999999999% a year (12 nines) | [B1] |
| 17 | Availability target | Over 99.99% | [B1] |
| 18 | Geographic copies | Every block in at least two zones, thousands of miles apart | [B2][B5] |
| 19 | Access latency | Every block readable in milliseconds, including old data | [B2] needs low-latency reads for every block. So archive tiers with hour-long restores are excluded [P2][P8] |

## 3. Base case, year by year (20% growth, 2 MB blocks, egress 2 x ingest)

All *(our calc)* from section 2.

| Year | Stored at start (PB) | Stored at end (PB) | Average stored (PB) | Ingest (PB) | Egress (PB) | PUTs (billions) | GETs (billions) | PUT/s | GET/s |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 500.0 | 600.0 | 550.0 | 100.0 | 200.0 | 50.0 | 100.0 | 1,585 | 3,171 |
| 2 | 600.0 | 720.0 | 660.0 | 120.0 | 240.0 | 60.0 | 120.0 | 1,903 | 3,805 |
| 3 | 720.0 | 864.0 | 792.0 | 144.0 | 288.0 | 72.0 | 144.0 | 2,283 | 4,566 |
| 4 | 864.0 | 1,036.8 | 950.4 | 172.8 | 345.6 | 86.4 | 172.8 | 2,740 | 5,479 |
| 5 | 1,036.8 | 1,244.2 | 1,140.5 | 207.4 | 414.7 | 103.7 | 207.4 | 3,288 | 6,575 |
| **Total** | | | | **744.2** | **1,488.3** | **372.1** | **744.2** | | |

- At the baseline, 83.3 PB is hot (written in the previous year) and 416.7 PB is cold.
- Each year, the previous year's ingest moves from hot to cold. That is one lifecycle transition per block, priced per request on S3 [P1].
- Cold retrievals in year 1: 10% of 200 PB = 20 PB.
- The high-growth case (41% a year) ends year 5 at about 2,800 PB instead of 1,244 PB.

## 4. Scale sweep, year 1

Same ratios as the base case, only the starting size changes. All *(our calc)*.

| Starting size | Average stored | Ingest | Egress | Blocks stored | PUT/s | GET/s |
|---|---:|---:|---:|---:|---:|---:|
| 100 TB | 110 TB | 20 TB | 40 TB | 50 million | 0.32 | 0.63 |
| 1 PB | 1.1 PB | 200 TB | 400 TB | 500 million | 3.2 | 6.3 |
| 10 PB | 11 PB | 2 PB | 4 PB | 5 billion | 32 | 63 |
| 100 PB | 110 PB | 20 PB | 40 PB | 50 billion | 317 | 634 |
| 500 PB | 550 PB | 100 PB | 200 PB | 250 billion | 1,585 | 3,171 |

#26 runs every size through all five years.

## 5. How #23 to #26 should use this

**Cloud (#23, #24)**, base case:
- **Hot data:** S3 Standard [P1] and Azure Hot ZRS [P5]. ZRS because S3 Standard spans at least 3 AZs [P2][P9].
- **Cold data:** S3 Standard-IA [P1] and Azure Cool ZRS [P5]. Both keep millisecond reads (row 19). Their 30-day minimums don't bite, since nothing is deleted (row 15).
- **Cheaper cold option:** S3 Glacier Instant Retrieval and Azure Cold ZRS. They charge less to store but more to retrieve and to write [P1][P5]. Price this option too: with only 10% of reads hitting cold data (row 14), it may win.
- **Requests:** PUTs and GETs from section 3, plus one lifecycle transition per block per year. On-prem pays nothing per request.
- **Egress:** priced on the internet bands [P1][P6]. Egress also stands in for serving users from the cloud region *(our inference)*.
- **Single region by default.** Magic Pocket keeps two geographic copies (row 18), which a single cloud region does not match. The second-copy option is a scenario for #26: double the storage and pay inter-region transfer on ingest ($0.086/GB on AWS [P1], $0.08 on Azure [P6]).

**On-prem (#25):**
- **Usable capacity:** the stored data in section 3.
- **Raw capacity:** usable x 3.0 *(our calc)*. That is two geographic copies [B2] x RS(6,3) erasure coding at 1.5x [T8][T9].
  - Optional saving: data on Dropbox's cold tier needs only 2.25x *(our calc)*: the cold tier's 1.5x across regions [B5] x RS(6,3) at 1.5x [T8][T9].
  - The cold tier only arrived in 2019 [B5], and the 2016 coding scheme was never published [B3]. So 3.0x is the 2016 base and 2.25x is a later optimisation scenario *(our inference)*.
- **Network:** IP transit sized for the egress in section 3 [H9].

## 6. Sanity checks

- **Storage list price vs Dropbox's reported spend.** 500 PB is about 465.7 million GiB. At today's S3 Standard Mumbai prices [P1] that costs about **$10.7M a month, $128.5M a year**, before requests or egress *(our calc)*. Dropbox's third-party infrastructure spend fell by $92.5M in 2016 as it moved off [F1]. The two are the same order of magnitude, which suggests the baseline size is right. Dropbox's actual contract prices are not public.
- **Object count.** 250 billion blocks in 2016 (row 6) against "trillions of blobs" in 2026 [B15], after the data grew several times [T5][T7].
- **Throughput headroom.** Dropbox provisions storage at "30Gbps per PB" [B14]: 15 Tb/s at 500 PB. Our base egress averages about 51 Gb/s in year 1: 200 PB over a year *(our calc)*. A provisioning standard covers peaks, repair and internal traffic, so being well under it is expected.

## 7. Uncertainties

- **500 PB or 600 PB.** B1 (2016) says 500 PB of user data. B14 (2025) says "roughly 600PB" was stored at the time. Using 600 PB would scale every volume in section 3 by 1.2.
- **Request rates are the weakest number.** They are derived from assumptions (rows 5 and 9). Dropbox's only published rates are far higher: "tens of millions of requests per second" in 2022 [T8]. But those count internal traffic across a much larger system. #23 and #24 should price GETs at both the base rate and the upper bound in row 11.
- **Egress is an assumption (row 9).** It moves the cloud bill more than any other parameter. Run it at 1x, 2x and 5x ingest.
- **Block size (row 5).** Smaller blocks mean more requests for the same data. At 1 MB, request counts double. At 4 MB, they halve.
- **No deletes (row 15).** This overstates storage and understates write churn.
- **2026 prices for a 2016 workload.** This answers "what would this workload cost today", not "what did Dropbox pay in 2016".
