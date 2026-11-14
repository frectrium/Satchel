# Azure Pricing Calculator estimate

Issue #24. The case workload (#15) priced on Azure Blob Storage in Central India. The breakdown and sensitivity cases match the AWS estimate in [../aws/README.md](../aws/README.md) (#23).

- **Estimate link:** not saved yet. The calculator only saves and shares an estimate after signing in to an Azure account [P7]. Everything it holds is in the export below
- **Export:** [`azure-estimate.json`](azure-estimate.json) in this folder: the estimate exactly as the calculator sends it to its Excel export, with every line, its settings and its price. The Excel file itself couldn't be saved from the browser used
- **Created:** 2026-11-12, by @JasMehta08

## AWS to Azure mapping

| AWS choice (#23) | Azure equivalent | Same or different |
|---|---|---|
| Region ap-south-1 (Mumbai) | Central India | Closest comparable region |
| S3 Standard, at least 3 AZs [P2] | Hot tier, **ZRS**, three or more AZs [P9] | Same redundancy. LRS would be one datacenter and is cheaper, so it isn't comparable |
| S3 Standard-IA, at least 3 AZs, 30-day minimum [P1][P2] | Cool tier, ZRS, 30-day minimum [P8] | Same storage price, $0.0138 per GB-month [P1][P5] |
| PUT and GET per 1,000 [P1] | Write and read operations per 10,000 [P5] | Units differ. The table below is already in Azure's units |
| Lifecycle transition into Standard-IA [P1] | Tier change from Hot to Cool, entered as Cool write operations | Our reading of how Azure bills tier changes. Check that the calculator agrees |
| Data transfer out to the internet [P1] | Bandwidth: internet egress, Microsoft network routing [P6] | Separate service in the calculator |

## How the estimate is set up

1. Open https://azure.microsoft.com/en-us/pricing/calculator/. No account is needed.
2. Make five groups named `Year 1` to `Year 5`, as in #23.
3. In each group, add **Storage Accounts** twice, once for Hot and once for Cool:
   - Region: **Central India**
   - Type: **Block Blob Storage**, Performance **Standard**, Account type **General Purpose V2**, File structure **Flat namespace**
   - Access tier: **Hot** for the first, **Cool** for the second. Redundancy: **ZRS** for both
4. Add **Bandwidth** to each group:
   - Data transfer out, internet egress
   - Routing through the **Microsoft Premium Global Network**
   - Source region Central India
5. Enter that year's column from the table below. Capacity is in TB, the same binary unit as #23 *(our inference)*. Leave list, create container and other operations at 0.
6. Save, copy the share link here, and export the estimate into this folder.

## What to enter (per month)

| Field (per month) | Year 1 | Year 2 | Year 3 | Year 4 | Year 5 |
|---|---:|---:|---:|---:|---:|
| Hot ZRS: capacity (TB) | 83,370.3 | 100,044.4 | 120,053.3 | 144,064.0 | 172,876.8 |
| Hot ZRS: write operations (x 10,000) | 416,667 | 500,000 | 600,000 | 720,000 | 864,000 |
| Hot ZRS: read operations (x 10,000) | 750,000 | 900,000 | 1,080,000 | 1,296,000 | 1,555,200 |
| Cool ZRS: capacity (TB) | 416,851.7 | 500,222.1 | 600,266.5 | 720,319.8 | 864,383.8 |
| Cool ZRS: write operations (x 10,000), tier changes from Hot | 347,222 | 416,667 | 500,000 | 600,000 | 720,000 |
| Cool ZRS: read operations (x 10,000) | 83,333 | 100,000 | 120,000 | 144,000 | 172,800 |
| Cool ZRS: data retrieval (TB) | 1,515.8 | 1,819.0 | 2,182.8 | 2,619.3 | 3,143.2 |
| Bandwidth: data transfer out to the internet (TB) | 15,158.2 | 18,189.9 | 21,827.9 | 26,193.4 | 31,432.1 |

## What the calculator should show

Expected cost *(our calc)* at the list prices in [P5] and [P6]. If a group differs by more than 2%, note the difference and its reason below.

| Component (per month) | Year 1 | Year 2 | Year 3 | Year 4 | Year 5 |
|---|---:|---:|---:|---:|---:|
| Hot ZRS capacity | $1,964,102 | $2,356,809 | $2,828,059 | $3,393,558 | $4,072,156 |
| Cool ZRS capacity | $5,890,615 | $7,068,738 | $8,482,486 | $10,178,983 | $12,214,780 |
| Hot write operations | $28,646 | $34,375 | $41,250 | $49,500 | $59,400 |
| Hot read operations | $3,300 | $3,960 | $4,752 | $5,702 | $6,843 |
| Cool read operations | $833 | $1,000 | $1,200 | $1,440 | $1,728 |
| Cool data retrieval | $15,522 | $18,626 | $22,352 | $26,822 | $32,187 |
| Tier changes to Cool | $34,722 | $41,667 | $50,000 | $60,000 | $72,000 |
| Internet egress | $1,242,574 | $1,490,927 | $1,788,950 | $2,146,578 | $2,575,732 |
| **Total per month** | **$9,180,315** | **$11,016,103** | **$13,219,049** | **$15,862,584** | **$19,034,825** |
| **Total per year** | **$110,163,777** | **$132,193,235** | **$158,628,584** | **$190,351,002** | **$228,417,905** |

Five-year total: **$819,754,502**.

## Differences from the expected figures

Every year matches the expected total to within $1, once the egress the calculator can't hold is added.

| Per month | In the calculator | Egress above 500,000 GB at $0.08 [P6] | Total | Expected |
|---|---:|---:|---:|---:|
| Year 1 | $7,978,551.42 | $1,201,763.44 | $9,180,314.86 | $9,180,315 |
| Year 2 | $9,565,986.80 | $1,450,116.08 | $11,016,102.88 | $11,016,103 |
| Year 3 | $11,470,909.29 | $1,748,139.36 | $13,219,048.65 | $13,219,049 |
| Year 4 | $13,756,816.32 | $2,105,767.20 | $15,862,583.52 | $15,862,584 |
| Year 5 | $16,499,904.74 | $2,534,920.64 | $19,034,825.38 | $19,034,825 |

Over 5 years that is $819,754,503.35, against the expected $819,754,502.

- **Egress limit.** The calculator takes at most 500,000 GB of internet egress on one line [P7]. Each year's line holds 500,000 GB ($40,811.00 a month), and the rest is added at $0.08 per GB, the band it falls in [P6]. Splitting it over several lines would restart the volume bands, so it isn't done that way.
- **Egress bands.** The calculator charged the first 500,000 GB exactly as the API thresholds give: free up to 100 GB, then $0.12, $0.085, $0.082 and $0.08 per GB [P6]. It names the routing option "Microsoft Global Network".
- **Tier changes.** The calculator has no field for tier changes. They are entered as Cool write operations, $0.1 per 10,000 [P5], as planned.
- **Groups.** The Azure calculator has no groups, so each line is named by year instead, for example "Year 1: Hot ZRS".
- **Units.** Capacity and retrieval are in TB, the calculator's 1,024 GB [P4], to three decimals.

## Sensitivity cases

The same four cases as #23: growth 10% and 30%, egress 1x and 3x ingest. `satchel-cost` (#52) produces them.
