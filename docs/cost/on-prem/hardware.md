# On-prem estimate: hardware

Issue #25, hardware part. What it costs to buy the storage servers, drives and switches to own the case workload (#15) for 5 years. Power, space and network transit (Guru) and staff and amortization (Jas) are the other two parts of #25, and they build on the counts here.

The numbers come from [`hardware.py`](hardware.py). Run `python3 docs/cost/on-prem/hardware.py` to regenerate [`hardware-inputs.csv`](hardware-inputs.csv) (every input, with its source) and [`hardware.csv`](hardware.csv) (every scale and year).

## Method

1. **How much raw capacity.** Data stored at the end of each year, from the workload model: 500 PB growing 20% a year [B1][T5]. Two regions, each RS(6,3), gives 3.0x raw [B2][T8][T9], as in #15.
2. **How many servers.** Magic Pocket's current hardware: 32 TB drives [B14], 100 per server [B9], so 3.2 PB raw per server, and 8 servers per rack [B9]. Whole servers, at least 9 per region so RS(6,3) can put each fragment on its own server *(our inference)*. These are the same rules as the sizing model (#53).
3. **What a server costs.** Backblaze's Storage Pod 6.0 stored 1 PB for about $35,000, drives included, with a low end of $0.032 per GB [H3]. We price installed raw capacity at $35 per TB (base) and $32 per TB (low).
4. **Switches.** One 32-port 100GbE top-of-rack switch per rack, with 2 x 100G per server down and 16 ports up, and enough spine switches for every rack uplink *(our inference: a non-blocking leaf-spine)*. $7,432.57 per switch [H14]. Optics and cables aren't priced.
5. **When it's bought.** At the start of each year, enough for the data at its end *(our inference)*. The horizon is 5 years, within a 5 to 6 year server life [H13], so nothing is replaced.

## Base case: 500 PB

| Year | Stored at end (PB) | Raw needed (PB) | Servers | Racks | Switches | Servers bought | Server cost, base | Switch cost |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 600.0 | 1,800.0 | 564 | 72 | 108 | 564 | $63.2M | $0.80M |
| 2 | 720.0 | 2,160.0 | 676 | 86 | 130 | 112 | $12.5M | $0.16M |
| 3 | 864.0 | 2,592.0 | 810 | 102 | 154 | 134 | $15.0M | $0.18M |
| 4 | 1,036.8 | 3,110.4 | 972 | 122 | 184 | 162 | $18.1M | $0.22M |
| 5 | 1,244.2 | 3,732.5 | 1,168 | 146 | 220 | 196 | $22.0M | $0.27M |
| **Total** | | | | | | **1,168** | **$130.8M** | **$1.64M** |

Hardware over 5 years: about **$132.5M** at the base price, $121.2M at the low one (our calc). At the end, 116,800 drives fill 146 racks. At Dropbox's 15 kW per-rack budget [B14], that's up to 2,190 kW of IT load for the power part of #25 (our calc).

## Scale sweep

The same 5 years for each starting size in #15. Whole servers and the 9-per-region minimum make small clusters cost far more per TB than the data needs.

| Start | Servers in year 5 | Racks | Switches | Hardware, 5 years | Raw needed in year 5 x $35 per TB |
|---|---:|---:|---:|---:|---:|
| 100 TB | 18 | 4 | 6 | $2.06M | $0.03M |
| 1 PB | 18 | 4 | 6 | $2.06M | $0.26M |
| 10 PB | 24 | 4 | 6 | $2.73M | $2.61M |
| 100 PB | 234 | 30 | 46 | $26.5M | $26.1M |
| 500 PB | 1,168 | 146 | 220 | $132.5M | $130.6M |

The last column prices only the raw capacity the data needs, as if servers came in any size. The truth for a small buyer lies between the two columns, since smaller servers exist. That's the main reason owning loses at small scale *(our inference)*. The break-even analysis (#26) should run both.

## How reliable this is

- **The server price is from 2019** [H3]. Drives alone cost $27.92 to $44.17 per TB in August 2026 [H2], which is 80% to 126% of the 2019 all-in figure. So $35 per TB likely understates a 2026 server *(our inference)*. #26 should test a higher server price. A current vendor quote would replace this.
- **The switch price is one reseller listing** [H14] and doesn't say what's included.
- **Not included:** spare drives and servers (no Dropbox figure), optics and cables, racks, PDUs, installation and shipping, and taxes.
- **Dropbox's hardware is custom** [B4][B9]. Its real cost per TB isn't public, and at its scale it's likely lower than a Storage Pod's *(our inference)*.
