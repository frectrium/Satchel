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
| | | | | | | | |

## H: Hardware and on-prem prices

| ID | Item | Vendor / source | Price | URL | Archive | Accessed | Key points |
|---|---|---|---|---|---|---|---|
| H1 | Hard drive cost per gigabyte (Backblaze blog, Andy Klein, 2022-11-29) | Backblaze | $0.0144 per GB ($14.40 per TB) | https://www.backblaze.com/blog/hard-drive-cost-per-gigabyte/ | https://web.archive.org/web/20261021052751/https://www.backblaze.com/blog/hard-drive-cost-per-gigabyte/ | 2026-10-20 | Average cost of the 16 TB drives Backblaze was buying in Nov 2022. The post projected $0.01 per GB around mid-2025 with 22 or 24 TB drives. Historical baseline, not a current price |
| H2 | HDD price per TB, March vs August 2026 (eRacks press release, 2026-09-14) | eRacks (system integrator) | $27.92 to $44.17 per TB (Aug 2026, by drive line) | https://www.pr.com/press-release/978414 | https://web.archive.org/web/20261021052828/https://www.pr.com/press-release/978414 | 2026-10-20 | Seagate Exos SATA $24.33 to $27.92 per TB (+15%). Toshiba MG enterprise $22.17 to $30.64 (+38%). WD Gold $29.79 to $44.17 (+48%). Integrator's own tracking, retail-level prices |
| H3 | Petabytes on a Budget: 10 Years and Counting (Backblaze blog, Andy Klein, 2019-09-24) | Backblaze | About $35,000 per PB (Storage Pod 6.0) | https://www.backblaze.com/blog/petabytes-on-a-budget-10-years-and-counting | https://web.archive.org/web/20261021052858/https://www.backblaze.com/blog/petabytes-on-a-budget-10-years-and-counting/ | 2026-10-20 | Storage Pod 1.0 stored 1 PB for about $81,000, Storage Pod 6.0 for about $35,000. With 12 TB drives, about $0.032 to $0.034 per GB including the drives. Built by a contract manufacturer |
| H4 | Electric Power Monthly, Table 5.6.A (US EIA), July 2026 data | US Energy Information Administration | 14.53 cents per kWh (US commercial), 9.77 (US industrial) | https://www.eia.gov/electricity/monthly/epm_table_grapher.php?t=epmt_5_6_a | https://web.archive.org/web/20261021052936/https://www.eia.gov/electricity/monthly/epm_table_grapher.php?t=epmt_5_6_a | 2026-10-20 | Average price to ultimate customers, July 2026. Commercial / industrial: Virginia 11.68 / 10.07, Texas 8.47 / 7.07, Oregon 10.60 / 8.92, California 30.56 / 25.11 cents per kWh |
| H5 | MERC press note on the MSEDCL Multi-Year Tariff order, FY 2025-26 to 2029-30 (order dated 2025-03-28) | Maharashtra Electricity Regulatory Commission (copy hosted by HRAWI) | Rs 7.48 per unit + Rs 555 per kVA per month (HT industry, FY 2025-26) | https://www.hrawi.com/pdf/Press-Note_MYT-Order_MSEDCL_English-2.pdf | https://web.archive.org/web/20261021053037/https://www.hrawi.com/pdf/Press-Note_MYT-Order_MSEDCL_English-2.pdf | 2026-10-20 | HT I(A) Industry: Rs 7.48 / 7.27 / 7.11 per unit and Rs 555 / 560 / 565 per kVA per month for FY 2025-26 / 26-27 / 27-28. HT II Commercial: Rs 9.26 / 8.83 / 8.72 per unit, same demand charges. Data centres are placed in the industrial category |
| H6 | Uptime Institute Global Data Center Survey 2025: regional view (UII Data Report 183, Aug 2025) | Uptime Institute | Average PUE 1.54 (global) | https://intelligence.uptimeinstitute.com/sites/default/files/2025-08/UI%20Field%20report%20183_Annual%20survey%20-%20regional%20view%202025.pdf | https://web.archive.org/web/20260508101022/https://intelligence.uptimeinstitute.com/sites/default/files/2025-08/UI%20Field%20report%20183_Annual%20survey%20-%20regional%20view%202025.pdf | 2026-10-20 | Average annual PUE 2025: global 1.54 (n=681), Asia-Pacific excluding China 1.52 (n=79), North America 1.49 (n=111). The main 2025 survey says average PUE has barely changed for six years. Original URL returned 404 on 2026-10-21, use the archive copy |
| H7 | North America Data Center Trends H1 2026: Northern Virginia (CBRE) | CBRE | $160 to $185 per kW | https://www.cbre.com/insights/books/north-america-data-center-trends-h1-2026/northern-virginia-data-center-market | n/a (site blocks Wayback saves, 2026-10-21) | 2026-10-20 | Asking rates for 10+ MW wholesale requirements in Northern Virginia. The page does not state the time unit; market convention is per kW per month (our inference) |
| H8 | Full rack colocation pricing from top Indian data centers (Cyfuture knowledge base, undated, 2026 page) | Cyfuture (Indian colocation provider) | Rs 25,000 to 60,000 per rack per month, plus Rs 8,000 to 20,000 per kVA per month for power | https://cyfuture.cloud/kb/colocation/full-rack-colocation-pricing-from-top-indian-data-centers | https://web.archive.org/web/20261021053230/https://cyfuture.cloud/kb/colocation/full-rack-colocation-pricing-from-top-indian-data-centers | 2026-10-20 | Power is billed per kVA on top of rack rental. Full racks draw 2 to 8 kVA. Premium Mumbai or Bengaluru racks can pass Rs 75,000. Table of provider ranges (Netmagic/NTT, Sify, CtrlS). Vendor article, so treat as indicative |
| H9 | IP Transit Pricing in 2026: More Competition, More Price Erosion (TeleGeography, Brianna Boudreau, 2026-09-16) | TeleGeography | $0.03 per Mbps per month (lowest 100 GigE) | https://resources.telegeography.com/ip-transit-price-erosion-significant-regional-differences-remain | https://web.archive.org/web/20261021053303/https://resources.telegeography.com/ip-transit-price-erosion-significant-regional-differences-remain | 2026-10-20 | Lowest 100 GigE price $0.03 per Mbps per month (Q2 2026). Lowest 10 GigE $0.07. 400 GigE weighted median $0.08 in key US and European cities. Mumbai 100 GigE prices fell 35% a year (compounded) from 2023 to 2026. City medians are only in a chart |
| H10 | Cross connect from Equinix DC3 Ashburn (Voxility price list) | Voxility (reseller), Equinix facility | $1,000 setup + $375 per month | https://www.voxility.com/cross-connects/prices/Cross+Connect+from+Equinix+DC3+Ashburn?export=PDF | n/a (site blocks Wayback saves, 2026-10-21) | 2026-10-20 | Single-mode fibre pair. Prices valid at 2026-07-16, excluding taxes. Reseller price, not Equinix's own list |
| H11 | Occupational Outlook Handbook: Network and Computer Systems Administrators (US BLS) | US Bureau of Labor Statistics | $99,130 per year (median, May 2025) | https://www.bls.gov/ooh/Computer-and-Information-Technology/network-and-computer-systems-administrators.htm | https://web.archive.org/web/20261021053501/https://www.bls.gov/ooh/Computer-and-Information-Technology/network-and-computer-systems-administrators.htm | 2026-10-20 | Median annual wage in May 2025. Base wage only, no benefits or overhead. Closest BLS category to a storage operations engineer (our inference) |
| H12 | Site Reliability Engineer (SRE) salary in India (PayScale) | PayScale | Rs 17,48,344 per year (average base) | https://www.payscale.com/research/IN/Job=Site_Reliability_Engineer_(SRE)/Salary | https://web.archive.org/web/20261021053624/https://www.payscale.com/research/IN/Job=Site_Reliability_Engineer_(SRE)/Salary | 2026-10-20 | 10th to 90th percentile Rs 4.05 lakh to Rs 30 lakh. 272 salary profiles. Updated 2026-06-24. Self-reported data |
| H13 | Amazon.com Form 10-K for FY2025 (filed 2026-02-06), property and equipment useful lives | Amazon (SEC filing) | 5 to 6 years (servers and networking equipment) | https://www.sec.gov/Archives/edgar/data/1018724/000101872426000004/amzn-20251231.htm | https://web.archive.org/web/20261006152835/https://www.sec.gov/Archives/edgar/data/1018724/000101872426000004/amzn-20251231.htm | 2026-10-20 | Useful life of servers raised from five to six years from 2024-01-01. From 2025-01-01, a subset of servers and networking equipment cut from six to five years |

## O: Other

| ID | Title | Author(s) | Date | URL | Accessed | Key points |
|---|---|---|---|---|---|---|
| | | | | | | |
