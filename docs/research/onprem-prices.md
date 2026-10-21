# On-prem price references

Issue #7. Collected 2026-10-20. These are the public prices we can use to estimate what it costs to **own** storage infrastructure, for the on-prem side of the cost comparison (#25, #26). Each figure has its Source Log ID. Nothing here is a Dropbox number: Dropbox's own costs come from the S-1 and 10-K work (#4).

Prices are in the source's currency. We don't convert INR to USD here, because the exchange rate will be fixed once for the whole cost model *(our inference)*.

## 1. Hardware

| Item | Price | Unit | Date of price | Source |
|---|---|---|---|---|
| HDD, average paid by Backblaze (16 TB drives) | $0.0144 | per GB | Nov 2022 | [H1] |
| HDD, Backblaze projection for 22 to 24 TB drives | $0.01 | per GB | projected for mid-2025 | [H1] |
| HDD, Seagate Exos SATA (enterprise) | $27.92 (was $24.33 in March) | per TB | Aug 2026 | [H2] |
| HDD, Toshiba MG (enterprise) | $30.64 (was $22.17 in March) | per TB | Aug 2026 | [H2] |
| HDD, WD Gold (enterprise) | $44.17 (was $29.79 in March) | per TB | Aug 2026 | [H2] |
| Storage server with drives, Backblaze Storage Pod 1.0 | about $81,000 | per PB | 2009 design | [H3] |
| Storage server with drives, Backblaze Storage Pod 6.0 | about $35,000 | per PB | 2016 design, 2019 prices | [H3] |
| Storage Pod 6.0 with 12 TB drives | $0.032 to $0.034 | per GB | Sep 2019 | [H3] |

Notes:
- Drive prices are rising in 2026, not falling. Every drive line eRacks tracked went up between March and August 2026, by 15% to 92% [H2]. This is the opposite of the trend Backblaze projected in 2022 [H1].
- The most recent full-server cost we found (drives, chassis, CPU, network) is the 2019 Storage Pod figure [H3]. Backblaze hasn't published a newer per-PB figure in the posts we read. A current number needs a newer source, for example a vendor quote for a 60 or 90 bay storage server.
- We have no source yet that splits a storage server's cost into drives and everything else. We need one to scale the 2019 server figure to 2026 drive prices.

## 2. Power

### 2.1 Electricity price

| Location | Sector | Price | Date | Source |
|---|---|---|---|---|
| US average | Commercial | 14.53 cents per kWh | July 2026 | [H4] |
| US average | Industrial | 9.77 cents per kWh | July 2026 | [H4] |
| Virginia (largest US data center market) | Commercial / Industrial | 11.68 / 10.07 cents per kWh | July 2026 | [H4] |
| Oregon | Commercial / Industrial | 10.60 / 8.92 cents per kWh | July 2026 | [H4] |
| Texas | Commercial / Industrial | 8.47 / 7.07 cents per kWh | July 2026 | [H4] |
| California | Commercial / Industrial | 30.56 / 25.11 cents per kWh | July 2026 | [H4] |
| Maharashtra (MSEDCL), HT I(A) Industry | Energy + demand charge | Rs 7.48 per unit + Rs 555 per kVA per month | FY 2025-26 | [H5] |
| Maharashtra (MSEDCL), HT I(A) Industry | Energy + demand charge | Rs 7.27 per unit + Rs 560 per kVA per month | FY 2026-27 | [H5] |
| Maharashtra (MSEDCL), HT II Commercial | Energy + demand charge | Rs 9.26 per unit + Rs 555 per kVA per month | FY 2025-26 | [H5] |
| Maharashtra (MSEDCL), HT II Commercial | Energy + demand charge | Rs 8.83 per unit + Rs 560 per kVA per month | FY 2026-27 | [H5] |

Mumbai is where our AWS region sits. MERC places data centres in the industrial category, so HT I(A) is the relevant rate for a data centre there [H5].

### 2.2 PUE (facility overhead on IT power)

| Scope | Average annual PUE, 2025 | Sample size | Source |
|---|---|---|---|
| Global | 1.54 | 681 | [H6] |
| Asia-Pacific (excluding China) | 1.52 | 79 | [H6] |
| North America | 1.49 | 111 | [H6] |

Uptime's main 2025 survey report, which is not in the Source Log yet, says average PUE has barely moved for six years running (noted in [H6]). These are industry averages. Large operators may run lower, but we have no source for that yet.

## 3. Space (colocation)

| Market | Price | Unit | What's included | Source |
|---|---|---|---|---|
| Northern Virginia, wholesale 10+ MW | $160 to $185 | per kW (per month assumed, see notes) | Space, power capacity and cooling, priced per kW of critical load | [H7] |
| India, full rack | Rs 25,000 to 60,000 | per rack per month | Rack space only. Power billed separately | [H8] |
| India, premium rack in Mumbai or Bengaluru | Rs 75,000 or more | per rack per month | Rack space only | [H8] |
| India, power for a colocated rack | Rs 8,000 to 20,000 | per kVA per month | A full rack draws 2 to 8 kVA | [H8] |
| India, provider ranges (rack / power per kVA per month) | Netmagic (NTT): Rs 50,000+ / Rs 12,000 to 20,000. Sify: Rs 45,000 to 60,000 / Rs 10,000 to 18,000. CtrlS: Rs 55,000+ / Rs 15,000+ | | | [H8] |

## 4. Network

| Item | Price | Unit | Date | Source |
|---|---|---|---|---|
| IP transit, 100 GigE, lowest price in competitive markets | $0.03 | per Mbps per month | Q2 2026 | [H9] |
| IP transit, 10 GigE, lowest price | $0.07 | per Mbps per month | 2026 | [H9] |
| IP transit, 400 GigE, weighted median in key US and European cities | $0.08 | per Mbps per month | 2026 | [H9] |
| IP transit trend, Mumbai, 100 GigE | down 35% a year, compounded, 2023 to 2026 | | | [H9] |
| Cross-connect, Equinix DC3 Ashburn, single-mode fibre pair | $1,000 setup + $375 per month | per cross-connect | valid at 2026-07-16 | [H10] |

## 5. Staff

| Role | Location | Pay | Basis | Source |
|---|---|---|---|---|
| Network and computer systems administrator | US | $99,130 per year | Median annual wage, May 2025 | [H11] |
| Site reliability engineer | India | Rs 17,48,344 per year (range Rs 4.05 lakh to Rs 30 lakh) | Average base salary, self-reported, 272 profiles | [H12] |

Both figures are base pay only. The cost to the employer is higher once benefits, taxes and overhead are added, and we don't have a source for that multiplier yet.

## 6. Amortization

| Asset | Useful life | Who | Source |
|---|---|---|---|
| Servers and networking equipment | 5 to 6 years | Amazon, as of 2025-12-31 | [H13] |
| Servers | changed from 5 to 6 years from 2024-01-01 | Amazon | [H13] |
| Subset of servers and networking equipment | changed from 6 to 5 years from 2025-01-01 | Amazon | [H13] |

Dropbox's own depreciation periods for servers should come from its filings (#4), since that is the primary source for this case.

## 7. What is uncertain or varies widely

- **HDD prices are volatile right now.** Enterprise drive lines rose 15% to 48% between March and August 2026, and one consumer line rose 92% [H2]. Whatever $/TB we pick, the cost model should run a sensitivity range (for example the March and August 2026 values) *(our inference)*.
- **Server cost is old.** The only full-server $/PB figure is from 2019 [H3].
- **Indian electricity tariffs may have changed.** MERC suspended the March 2025 MSEDCL order for a time and issued a review order on 2026-03-25. The rates in [H5] are from the original March 2025 order, as summarised in its press note, and may differ from what applies today. The press note also says "Rs/unit" without stating whether a unit is a kWh or a kVAh. Next steps: link the official MERC copy instead of the HRAWI mirror, and check the current tariff schedule.
- **Colocation unit is not stated.** CBRE gives "$160 to $185 per kW" without "per month" [H7]. Per month is the usual convention, but we should confirm it. CBRE's figure is for 10+ MW deals. Smaller deployments pay more.
- **Indian colocation figures come from a provider's own article** [H8]. It is undated and gives ranges. Look for an analyst report (JLL, CBRE India, Cushman and Wakefield) with Mumbai rates per kW.
- **IP transit prices for Mumbai are not given as a number,** only as a rate of decline [H9]. City medians are only in a chart.
- **Cross-connect price is from a reseller,** for one US facility [H10]. Indian cross-connect pricing is still missing.
- **Staff figures are not storage-specific.** BLS has no "storage engineer" or "SRE" category, so the closest one is used [H11]. The Indian figure is self-reported [H12].
- **Amortization reference is Amazon's,** a hyperscaler with its own accounting choices [H13]. Dropbox's choice may differ.
