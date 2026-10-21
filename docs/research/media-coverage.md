# Media coverage of the S3 migration

Issue #5. Collected 2026-10-20.

Media coverage is **not** a primary source. We use it for context, to build a timeline, and to find primary sources we may have missed. No fact in our reports should rest on an M source alone: each claim below points to the primary source it should be checked against.

## 1. What we found

| ID | Outlet | Date | Angle |
|---|---|---|---|
| [M1] | Fortune | 2016-03-14 | Announcement day. Scale, network, what stays on AWS |
| [M2] | The Register | 2016-03-14 | Announcement day. Volume, timeline, durability target |
| [M3] | Computerworld | 2016-03-15 | Roundup of the first day's coverage, including Wired and Hacker News |
| [M4] | InfoQ | 2016-03-26 | Timeline taken from the Dropbox blog, with links to the primary posts |
| [M5] | GeekWire | 2018-02-23 | IPO filing: how much the move saved |
| [M6] | Data Center Knowledge | 2018-03-01 | IPO filing: savings and cost of revenue |
| [M7] | Data Center Knowledge | 2020-02-26 | Five years on: hardware cycles and rack density |
| [M8] | DatacenterDynamics | 2021-11-02 | Interview with a Dropbox engineer: hardware refresh, Rust, what still runs on AWS |

Coverage clusters around two moments: the March 2016 announcement and the February 2018 S-1 filing. Later pieces (2020, 2021) add operating detail from interviews.

### Not added yet

- **Wired, Cade Metz, March 2016**: "The Epic Story of Dropbox's Exodus From the Amazon Cloud Empire". This is the most-cited piece of coverage, and [M3] relies on it. Our fetch tool was blocked from opening wired.com, so it needs to be read by hand before it goes in the Source Log.
- **The Information** and **Forbes**: none found yet. The Information is paywalled.

## 2. Timeline as reported in the press

Dates here are as the media reported them. Each one needs confirming against the primary source in section 3.

| When | Event | Reported by |
|---|---|---|
| 2013 | Dropbox becomes a major S3 customer | [M8] (conflicts with [M3], which says Dropbox's files were on AWS from its first years) |
| Summer / Sept 2013 | Magic Pocket work begins | [M2], [M4] |
| Aug / Sept 2014 | Migration or dark launch begins, mirroring data from 2 regions | [M2], [M4] |
| 2015-02-27 | Dropbox starts serving file contents from its own infrastructure | [M3], [M4] |
| 2015-04-30 | Server installs begin in three regional locations | [M3] |
| 2015-10-07 | 90% of user data served from Magic Pocket | [M4] |
| 2016-03-14 | Public announcement on the Dropbox tech blog | [M1], [M2], [M4] |
| Q4 2016 | "Infrastructure Optimization" project completed | [M5] |
| 2018-02-23 | S-1 filed, with savings figures | [M5], [M6] |

## 3. Claims to check against primary sources

"Points to" names the primary source the claim should be checked against. Source IDs for those will come from #2 (blog posts) and #4 (S-1). Where the media disagree, the conflict is noted.

| # | Claim in the media | Reported by | Points to (primary) | Notes |
|---|---|---|---|---|
| 1 | About 90% of user data moved to Magic Pocket | [M1], [M2], [M4], [M7] | Dropbox tech blog, 2016-03-14, "Magic Pocket infrastructure" (#2) | |
| 2 | Data volume at migration was 500 PB | [M2], [M3], [M4] | Same blog post (#2) | **Conflict:** [M7] says about 600 PB |
| 3 | Storage grew from 40 PB to 500 PB; Dropbox grew 12x since 2012 | [M4] | Same blog post (#2) | |
| 4 | Launch on 2015-02-27; 90% milestone on 2015-10-07 | [M3], [M4] | Same blog post (#2) | [M7] gives February to October 2015 as the migration window, which is consistent |
| 5 | Work started in summer or September 2013 | [M2], [M4] | Same blog post (#2) | Wording differs ("coding began" vs "concept") |
| 6 | Durability target 99.9999999999%, availability over 99.99% | [M2], [M4] | Same blog post (#2) | Exact figure matters for the design doc |
| 7 | Three regional data centers | [M1], [M5], [M6] | Blog post (#2) and S-1 (#4) | **Conflict:** [M8] says four data centers by 2016. [M6] names California, Texas and Virginia |
| 8 | Network peaks at over half a terabit per second | [M1], [M3] | Blog post (#2) | |
| 9 | Operating costs down $74.6M over two years | [M5], [M6] | S-1, "Infrastructure Optimization" (#4) | |
| 10 | 2016: third-party datacenter spend down $92.5M, own data center costs up $53M, net $39.5M saving | [M5], [M6] | S-1 (#4) | [M6] also gives cost of revenue down $16.8M (4%) in 2016 |
| 11 | 2017: further $35.1M saving; cost of revenue down $21.7M (6%) | [M5], [M6] | S-1 (#4) | |
| 12 | Part of the 2016 saving came from deleting long-inactive users' data | [M6] | S-1 (#4) | No amount given. Matters for how much of the saving is due to Magic Pocket itself |
| 13 | Under 10% of storage still on AWS, mainly outside the US (Germany in [M1], [M4]; the UK, mainland Europe and Japan in [M8]) | [M1], [M4], [M5], [M8] | S-1 (#4); Dropbox business blog, Feb 2016, "Dropbox is growing in Europe" (linked from [M4]) | |
| 14 | Hardware is replaced every four years | [M8] | Look for a Dropbox tech blog post on hardware (#2) | Feeds the amortization period in the on-prem estimate (#7) |
| 15 | Drives went from 4 TB to 20 TB; SMR drives used | [M8] | Dropbox tech blog posts on SMR and hardware (#2) | |
| 16 | Hardware build time cut from six to three months; 100-cabinet plans now need 30 | [M7] | Interview quote. Look for a talk or blog post (#2, #3) | |
| 17 | Migration code rewritten from Go to Rust | [M8] | Dropbox tech blog or talks (#2, #3) | Relevant to our choice of Rust |
| 18 | Diskotech density and 3x to 5x better tail latency | [M3] (from Hacker News) | Forum comment only. Look for a blog post or talk (#2, #3) | Not usable unless a primary source confirms it |

## 4. Primary sources the coverage points to

These came up in the coverage and should be in the B or F sections of the Source Log (owners of #2 and #4):

- Dropbox tech blog, 2016-03-14, Magic Pocket infrastructure announcement (Akhil Gupta). Linked from [M4] as `https://blogs.dropbox.com/tech/2016/03/magic-pocket-infrastructure/`. The current address appears to be `https://dropbox.tech/infrastructure/magic-pocket-infrastructure`.
- Dropbox business blog, 2016-02, "Dropbox is growing in Europe" (AWS Germany partnership). Linked from [M4].
- Dropbox Form S-1, filed 2018-02-23, section on "Infrastructure Optimization" ([M5], [M6]).
- People named in the coverage, useful for finding talks (#3): Akhil Gupta (VP of infrastructure), Preslav Le (engineer, [M8]), Latane Garetson (data center physical infrastructure, [M7]).
