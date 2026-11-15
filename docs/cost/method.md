# Cost method

How `satchel-cost` (#52) prices the workload, so the numbers in #23, #24, #25 and #26 can be checked by hand. The workload is defined in [workload-model.md](../research/workload-model.md) (#15). Prices are in `crates/satchel-cost/prices/`, each value tagged with its Source Log ID.

## 1. Workload, per year

For year y = 1 to 5, with growth g (base 20% [T5]) and starting size S0 (base 500 PB [B1]):

| Quantity | Formula |
|---|---|
| Stored at start, end | S(y-1), S(y) = S(y-1) x (1 + g) |
| Average stored | (S(y-1) + S(y)) / 2 |
| Ingest | S(y) - S(y-1) |
| Egress | 2 x ingest (assumption from #15) |
| PUTs | ingest / average block size (2 MB) |
| GETs | egress / average block size |

Volumes in the workload model are decimal (1 PB = 10^15 bytes). The engine converts to each provider's billing unit before pricing. AWS uses 1 GB = 2^30 bytes [P1], so 1 PB = 931,323 GB.

## 2. Hot and cold

- **Hot data** was written in the last 12 months. At 20% growth that is g / (1 + g) = 1/6 of the average stored (#15, row 13).
- **Cold data** is the other 5/6.
- **Reads:** 90% of GETs and egress hit hot data and 10% hit cold data [B5].
- **Transitions:** each year, the blocks written the year before move from hot to cold, one lifecycle transition per block. In year 1 that is the 83.3 PB written before the baseline (#15, section 3).

## 3. Cloud scenarios (AWS, Azure)

Priced month by month, using each year's averages divided by 12, then multiplied by 12 for the year.

| Component | AWS | Azure |
|---|---|---|
| Hot storage | S3 Standard, banded [P1] | Hot ZRS, banded [P5] |
| Cold storage | S3 Standard-IA [P1] | Cool ZRS [P5] |
| Writes | PUT at the Standard rate [P1] | Hot ZRS write operations [P5] |
| Hot reads | GET at the Standard rate [P1] | Hot ZRS read operations [P5] |
| Cold reads | GET at the Standard-IA rate, plus retrieval per GB [P1] | Cool read operations, plus retrieval per GB [P5] |
| Transitions | Lifecycle transitions into Standard-IA [P1] | Tier changes, priced as Cool write operations *(our inference, check against the calculator)* |
| Egress | Internet bands, after the first 100 GB [P1] | Microsoft network bands [P6] |

Bands apply to the monthly total, each band at its own price. Alternatives the engine also prices: Glacier Instant Retrieval / Cold ZRS for cold data, and a second region (double the storage, plus inter-region transfer on ingest at $0.086/GB [P1] or $0.08/GB [P6]).

## 4. On-prem scenario

- **Raw capacity:** usable x 3.0. Magic Pocket's two geographic copies [B2], each RS(6,3) at 1.5x [T8][T9] *(our calc)*. Sized by the layout module (#53).
- **Hardware:** raw capacity x server cost per PB [H3], or drives at $/TB [H2] plus the rest of the server. #25 (hardware part) decides which.
- **Capital cost:**
  - Hardware is bought when capacity is needed and depreciated straight-line over 5 years (range 4 to 6) [H13][F2].
  - The annual cost of owning it is the annuity at the cost of capital. Base 9.34% [H18], proposed.
- **Power:** drive watts per TB [B12] x raw TB x PUE [H6] x hours x $/kWh [H4][H5].
- **Space:** colocation per kW [H7] or per rack plus per kVA [H8].
- **Network:** IP transit for peak egress [H9], plus cross-connects [H10].
- **Staff:** 8 to 10 engineers [T9] x salary [H11][H12] x employer overhead (1.0 until a source exists).
- **INR to USD:** converted once, at Rs 94.8606 per USD [H19].

## 5. Sensitivity

Each scenario is re-run with one input changed at a time:

- growth 10% and 30% (±50%, as #23 asks)
- egress 1x and 3x ingest (±50% on the base 2x)
- block size 1 MB and 4 MB
- the low and high values of each ranged on-prem input

## 6. Reference figures for tests

From the method above, at the #15 base case *(our calc)*:

| Check | Value |
|---|---|
| 500 PB, all S3 Standard, one month | $10,710,773 |
| AWS, year 1, per month | $9,172,090 |
| Azure, year 1, per month | $9,180,315 |
| AWS, 5 years | $819,023,325 |
| Azure, 5 years | $819,754,502 |

The engine's unit tests check these within $1. The calculator exports (#23, #24) are checked within 2%.
