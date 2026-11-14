# On-prem estimate: staff and amortization

Issue #25, staff and amortization part. It covers the people who run the storage, and what the hardware from the [hardware part](hardware.md) costs per year once it's spread over its life. Power, space and network transit (Guru) are the third part of #25.

The numbers come from [`staff-amortization.py`](staff-amortization.py). It reads [`hardware.csv`](hardware.csv) and writes [`staff-amortization-inputs.csv`](staff-amortization-inputs.csv) (every input, with its source) and [`staff-amortization.csv`](staff-amortization.csv) (every scale, year and sensitivity case).

```bash
python3 docs/cost/on-prem/hardware.py
python3 docs/cost/on-prem/staff-amortization.py
```

All amounts are 2026 US dollars, like the rest of the workload model (#15). Staff are priced at US pay, since Magic Pocket's regions are in North America [T8].

## Staff

| Who | FTE | Pay | Source |
|---|---:|---|---|
| Engineers who run the storage software | 9 (8 to 10) | $135,980 base | "about 8 to 10 engineers" in 2024 [T9]. US median for software developers, May 2025 [H16] |
| Data center technicians | 16.8 | $99,130 base | 2 people on site at all times in each of the 2 sites. Uptime recommends one to two qualified operators on site 24x7 [H17]. US median for systems administrators [H11] |

- **Technician FTE** *(our calc)*: 2 sites x 2 on site x 8,760 hours / 2,080 hours per FTE = 16.8. 2,080 is 40 hours x 52 weeks, with no leave, so this understates headcount.
- **Fully loaded cost:** base pay is 70.0% of what US private employers spend per employee, with benefits making up the other 30.0% [H15]. So each salary is divided by 0.70 *(our calc)*.
- **Total: about $4.13M a year**, the same in every year and at every scale *(our calc)*. The low case (8 engineers, 1 person on site) is $2.75M. The high case (10 engineers) is $4.33M.

The software team doesn't shrink for a smaller system. T9's 8 to 10 engineers ran an exabyte-scale system [T9], which suggests the team doesn't grow much with data either *(our inference)*. That makes staff a fixed cost, which matters most at small scale (see the sweep below).

## Amortization

Hardware is bought at the start of each year (hardware part, step 5). Each year's purchase is spread over its useful life, with a cost of capital, so a server bought in year 5 is only charged for the year it's used inside the horizon.

| Input | Base | Sensitivity | Source |
|---|---|---|---|
| Useful life | 5 years | 4 and 6 years | Dropbox depreciates datacenter equipment over 3 to 5 years, and moved some infrastructure from 4 to 5 years in 2024 [F3]. It used 3, then 4 years in 2018 [F2]. Amazon uses 5 to 6 years for servers [H13] |
| Cost of capital | 9.34% a year | 5.3% and 10.66%, plus 0% | WACC for US Software (System & Application), January 2026 [H18]. Low: Dropbox's weighted-average discount rate on its finance leases, which are mostly infrastructure, at the end of 2025 [F3]. High: WACC for US Software (Internet) [H18]. 0% is the cash view |

Three views of the same purchases *(our calc)*:

- **Cash:** capex in the year it's paid. With a 4-year life, the year-1 hardware is replaced in year 5.
- **Depreciation:** straight line over the life, as Dropbox reports it [F3].
- **Capital charge:** each purchase times the capital recovery factor `r / (1 - (1 + r)^-life)`, which is 0.259 at 9.34% over 5 years. This is the yearly payment that buys the hardware over its life, so it's the fair number to set against a cloud bill paid by the month.

**For the comparison (#26), use the capital charge plus staff** (plus power, space and network) as the on-prem cost per year. Cash is a second view, for the "how much money do we need up front" question.

## Base case: 500 PB

Hardware at the base price from the hardware part. Millions of dollars *(our calc)*.

| Year | Capex (cash) | Depreciation | Capital charge | Staff | Capital charge + staff |
|---:|---:|---:|---:|---:|---:|
| 1 | 63.97 | 12.79 | 16.59 | 4.13 | 20.73 |
| 2 | 12.71 | 15.34 | 19.89 | 4.13 | 24.02 |
| 3 | 15.19 | 18.37 | 23.83 | 4.13 | 27.96 |
| 4 | 18.37 | 22.05 | 28.59 | 4.13 | 32.72 |
| 5 | 22.22 | 26.49 | 34.35 | 4.13 | 38.49 |
| **5 years** | **132.45** | **95.04** | **123.25** | **20.67** | **143.92** |

Over 5 years at 500 PB, *(our calc)*:

| Case | Cash | Depreciation | Capital charge |
|---|---:|---:|---:|
| Base: 5-year life, 9.34% | $132.5M | $95.0M | $123.2M |
| 4-year life | $196.4M (year-1 hardware replaced in year 5) | $118.8M | $147.8M |
| 6-year life | $132.5M | $79.2M | $107.0M |
| 5.3% cost of capital (Dropbox's lease rate) | $132.5M | $95.0M | $110.7M |
| 10.66% cost of capital | $132.5M | $95.0M | $127.5M |
| No cost of capital | $132.5M | $95.0M | $95.0M |

The life of the hardware moves the result more than the cost of capital does. A 4-year life adds $24.6M to the capital charge over 5 years, while the whole sourced range of the cost of capital, 5.3% to 10.66%, spans $16.8M.

## Scale sweep

5-year totals at the base inputs, millions of dollars *(our calc)*.

| Start | Capital charge | Staff | Total | Staff share |
|---|---:|---:|---:|---:|
| 100 TB | 2.67 | 20.67 | 23.34 | 89% |
| 1 PB | 2.67 | 20.67 | 23.34 | 89% |
| 10 PB | 2.90 | 20.67 | 23.57 | 88% |
| 100 PB | 24.79 | 20.67 | 45.46 | 45% |
| 500 PB | 123.25 | 20.67 | 143.92 | 14% |

Below about 10 PB, staff is nearly all of the cost, and it's the same staff that runs 500 PB. That, and the whole-server minimum in the hardware part, is why owning loses at small scale here *(our inference)*.

## How reliable this is

- **The team size is from one talk** [T9], about the storage software only. It doesn't count hardware engineering, network, security or management, so the staff cost is a lower bound *(our inference)*.
- **A fixed team at every scale is our assumption.** A company storing 100 TB wouldn't hire 9 storage engineers. It would use fewer, or buy an appliance. #26 should test a smaller team at the low end of the sweep.
- **The technician count comes from a staffing guideline** [H17], not from Dropbox, and it's per site. Dropbox's real number of sites and on-site staff isn't public. Hands-on work grows with the number of drives, which this doesn't model.
- **US pay.** Indian pay [H12] is much lower. An Indian on-prem case would also need Indian power and colocation (the other part of #25) and an exchange rate, which isn't in the Source Log yet.
- **The cost of capital is an industry average** [H18], not Dropbox's own. Dropbox's lease rate [F3] is lower, because it's secured on the equipment.
- **No wage growth or inflation,** since everything is in 2026 dollars.
