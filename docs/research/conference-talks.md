# Conference talks: Magic Pocket

Research notes for [#3](https://github.com/frectrium/Satchel/issues/3). These are the relevant public talks by Dropbox engineers that we could locate about Magic Pocket, its migration, hardware and operation.

- **Sources:** `T1` to `T10` in the [Source Log](../sources/README.md#t-conference-talks). All are primary sources delivered by Dropbox engineers or published by the event organizer.
- **Accessed:** 2026-10-21.
- **Method:** searched the Dropbox Tech Blog references, event archives, speaker and title variants, and public video indexes. Timestamps are approximate and point to the start of the relevant passage. Video notes paraphrase the speakers rather than relying on automatic captions as exact quotations.
- **Coverage limits:** T1 has a public recording but no captions or transcript; its notes use only the event summary. T3 has an event abstract but no public recording. T7 is a slide deck rather than a recording. T10 is cataloged from its event page, but the recording was not analyzed and no claims from it are used in the case brief.

## Talks at a glance

| ID | Year | Talk | Best evidence for |
|---|---:|---|---|
| T1 | 2016 | Dropbox's exabyte storage system | Contemporary framing: durability, verification and migration scale |
| T2 | 2016 | Bridging the Safety Gap from Scripts to Full Auto-Remediation | Operational guardrails and staged automation |
| T3 | 2016 | Go-ing to Rust: Optimizing Storage at Dropbox | Diskotech and the Go-to-Rust decision |
| T4 | 2017 | Scaling for Exabytes: Dropbox | Migration network, deadline and performance outcome |
| T5 | 2018 | Dropbox Background on Magic Pocket | Architecture history, dark launch and migration mechanics |
| T6 | 2018 | Dropbox Competitive Advantages from Infrastructure | Cost, performance and agility rationale |
| T7 | 2019 | The Magic Pocket: How Dropbox is handling large data sets | Architecture and hardware snapshot |
| T8 | 2022 | Magic Pocket: Dropbox's Exabyte-Scale Blob Storage System | Current system behavior, erasure coding and operations |
| T9 | 2024 | Scaling Magic Pocket: Exabyte Scale Object Storage System | Production coding schemes, SMR adoption and failure drills |
| T10 | 2018 | Dropbox Network Infrastructure | Companion overview of the edge network serving Magic Pocket |

## Key points by talk

### T1: Dropbox's exabyte storage system

James Cowling, Data @Scale, 2016. The event summary describes a system designed around durability, simple operation and continuous verification. It says Dropbox moved 2 EB of data from S3. The linked recording runs 35:15, but neither the event page nor the recording exposes captions or a transcript, so no timestamped technical claims are taken from it. [T1]

The 2 EB figure should not be read as the amount of unique user data stored. The contemporary announcement reports 500 PB of user data, while the event summary says data *moved*. Copies, verification and repeated transfer can make those different quantities. That explanation is plausible but not confirmed by Dropbox. *(our inference)* [B1][T1]

### T2: Bridging the Safety Gap from Scripts to Full Auto-Remediation

David Mah, SREcon16 Europe, 2016.

- **01:38-03:46:** routine storage maintenance can cause permanent loss if automation deallocates or reformats the last good copy. The team treated zero data-loss incidents as the constraint. [T2]
- **04:41-07:10:** Dropbox introduced automation in stages: first operator-driven scripts, then machine-selected work with human authorization, then limited automatic execution after confidence grew. [T2]
- **08:23-09:04:** the safety model asks why an action should *not* run, then requires the automation to verify those conditions rather than trusting an operator's intent. [T2]
- **11:25-14:00:** the Naoru remediation system checked availability and imposed rate limits before taking action. [T2]
- **14:50-15:50:** the team removed per-action human authorization only for remediations that had earned confidence. Disk reformatting still required human authorization because of its destructive consequence. [T2]

**Satchel implication:** automatic repair should have explicit preconditions, bounded concurrency, an audit trail and a manual gate for destructive disk operations. [T2]

### T3: Go-ing to Rust: Optimizing Storage at Dropbox

Jamie Turner, QCon San Francisco, 2016. The event abstract says Dropbox co-designed Diskotech hardware and software in 2015, evaluated Go and Rust, and selected Rust for a fleet described as tens of thousands of servers and nearly one million disks. No public recording or transcript was found, so the abstract supports the language and scale decision but not more detailed claims. [T3]

### T4: Scaling for Exabytes: Dropbox

Jason Wilson, Network @Scale, 2017.

- **00:22-01:17:** Dropbox built a multi-terabit network in roughly a year to support the migration, with control, performance and cost as the main reasons. [T4]
- **02:05-02:56:** the network dark-launched in August 2014, began serving data exclusively in February 2015, and entered the Base Jump capacity push from April through October. An expiring large AWS storage commitment created a hard deadline. [T4]
- **19:21-20:05:** Dropbox still described AWS as a partner after the migration. The talk put the system near 700 PB and heading toward 1 EB, and reported median upload or download improvements from about 40% to 300% depending on user location. [T4]

The transfer-speed result is a network outcome, not proof that every request or every location improved by that amount. [T4]

### T5: Dropbox Background on Magic Pocket

James Cowling, Storage Field Day 15, 2018.

- **02:07-03:35:** the 2018 snapshot was more than 500 million users and 1 EB, with over 20% annual data growth, more than 50,000 servers, over 2 Tb/s of traffic and more than 12 MW of power. [T5]
- **03:57-06:30:** Dropbox began with its own metadata services and S3 for file content, then moved most content into three US regions while retaining at least two geographic copies. [T5]
- **10:00-12:10:** a Python prototype grew to roughly 30 PB; performance work moved through Go before storage-node work moved to Rust. [T5]
- **12:19-15:27:** the dark launch ran for six months. A staging bug discovered after roughly 40 days reset the validation clock even though it did not lose user data. Keeping the old and new copies simultaneously was expensive. [T5]
- **15:32-17:55:** the first user data was removed from S3 in February 2015. The migration exceeded 6 PB per day and consumed roughly 30 to 40 racks of new capacity per day. Dropbox migrated users individually, verified the new copies, then deleted the S3 copies. [T5]
- **18:40-19:20:** Dropbox continued using AWS deliberately, including for European data residency. [T5]
- **26:40-29:00:** the early layout kept four copies in each of two locations, for eight copies before erasure coding. Cowling described a later locally repairable code but did not disclose its exact production parameters in this talk. [T5]
- **29:00-31:05:** verification generated more work and code than the core storage path. Reads were sampled after one minute, one hour, one day, one week and one month to test recovery paths. [T5]

### T6: Dropbox Competitive Advantages from Infrastructure

James Cowling, Storage Field Day 15, 2018.

- **00:13-02:25:** Cowling grouped the benefits of owning infrastructure into cost, performance and agility. Purchase price alone was not the comparison; power, useful life, maintenance and replacement timing affected the whole-lifecycle economics. [T6]
- **05:45-07:35:** cost was the leading reason at this scale, while direct control also let Dropbox change the hardware and software together and improve product performance. [T6]
- **08:35-09:20:** operating its own system did not mean rejecting public cloud. Dropbox still chose third-party infrastructure when its economics or geographic reach fit the workload. [T6]

This talk supplies rationale, not audited savings. Dollar outcomes should come from the filings. [T6][F1]

### T7: The Magic Pocket: How Dropbox is handling large data sets

Said Babayev and Martin Meusburger, CS3, 2019. Slide references are from the event deck.

- **Slide 13:** more than 1,000 PB of user block data, more than three regions, over 600 million users, more than 12,000 OSD machines and more than 650,000 disks. [T7]
- **Slides 15-20:** Frontends route puts and gets through the Block Index and Replication Table; Masters and Volume Managers manage cells, repair and coding outside the request path. [T7]
- **Slide 30:** an SMR host held 100 14 TB disks in a 4U chassis, with eight chassis per rack, 50 Gbps host networking and a 100 Gbps rack uplink. [T7]
- **Slide 31:** the OSD wrote directly to raw zoned disks through `libzbc`, grouped four 256 MB zones into a 1 GB extent, and had moved from Go to Rust. [T7]

### T8: Magic Pocket: Dropbox's Exabyte-Scale Blob Storage System

Facundo Agriel, QCon San Francisco, 2022; recording published by InfoQ in 2023.

- **00:46-01:30:** Magic Pocket ran in three North American regions, handled tens of millions of requests per second and used more than 600,000 drives and thousands of compute machines. Availability was described as more than four nines. [T8]
- **17:38-18:47:** open volumes initially kept four replicas in each of two zones, for eight copies, then converted closed data to erasure coding. [T8]
- **20:24-23:20:** the talk used RS(6,3) at 1.5x storage and an LRC example with 12 data, two local and two global parity fragments at about 1.33x. It said Magic Pocket used something very similar to Azure's LRC, but did not explicitly identify those example parameters as the only production layout. [T8]
- **25:15-27:55:** cold storage used two data fragments plus cross-region parity to reduce three-region overhead from 2x to 1.5x, about a 25% saving. [T8]
- **28:14-29:40:** releases took roughly four weeks and advanced through durability stages, including a week of verification, with automatic aborts when checks failed. [T8]
- **31:29-36:00:** background work dominated traffic. Repairs ran at about four 1 to 2 GB extents per second with a less-than-48-hour target; migrations at hundreds of petabytes required free capacity and deliberately slow, reversible execution. [T8]
- **38:16-39:03:** S3 still served regions outside the North American Magic Pocket footprint, including Europe, for locality and compliance. [T8]

### T9: Scaling Magic Pocket: Exabyte Scale Object Storage System

Sandeep Ummadi and Eric Shobe, SNIA Storage Developer Conference, 2024.

- **00:38-01:36:** Dropbox reported more than 700 million registered users, more than 100 million monthly active users and over one billion files uploaded daily. [T9]
- **03:15-03:40:** the service used geographic replication, targeted more than four nines of availability and more than eleven nines of durability, and moved tens of terabits per second. [T9]
- **06:00-08:25:** buckets had grown to 1 to 4 GB. The speaker described 4x initial replication before conversion to either RS(6,3) or LRC(12,2,2), and cells held about 100 to 120 PB. This is the clearest public statement of the 2024 production coding choices. [T9]
- **18:00-20:10:** host-managed SMR offered roughly 20% to 25% more density. Development began in 2015, and the raw-disk storage path became an early Rust production use at Dropbox after Go runtime limitations. [T9]
- **23:20-26:00:** removing the SSD cache raised per-host puts from about 500 to 1,500 per second and reduced p99 write latency from about 1.2 seconds to about 200 ms while removing a failure-prone part. [T9]
- **26:09-32:40:** a team of about 8 to 10 engineers operated the storage software. It tested every invariant weekly, tested the verifiers themselves, and automatically took a full zone offline for about 30 minutes each week. [T9]
- **40:50-42:30:** SMR represented about 98% of the fleet and supplied a 10% to 20% capacity benefit. The speakers date development to 2015 and production deployment of 14 TB SMR drives to 2018; these describe different stages rather than conflicting launch dates. [T9]

### T10: Dropbox Network Infrastructure

Dzmitry Markovich, Storage Field Day 15, 2018. This companion presentation covers the edge network that served Dropbox's storage system. The event page says Dropbox initially placed EC2 instances close to users, then developed its current network to improve latency and packet loss. The recording is linked for completeness, but it was not analyzed for timestamped claims and is not evidence for any claim in the case background. [T10]

## Updates and apparent contradictions

| Topic | Earlier statement | Later statement | Reading |
|---|---|---|---|
| Migration volume | T1's event summary says 2 EB moved; B1 says the estate held 500 PB | B14 retrospectively says more than 90% of about 600 PB moved in 2015 | Data transferred is not the same measure as unique stored data. Replicas or repeated movement may explain T1, but Dropbox does not reconcile the figures. [T1][B1][B14] |
| Erasure-code parameters | B3 and T5 describe an LRC-like code without exact parameters | T8 gives illustrative RS and LRC layouts; T9 names RS(6,3) and LRC(12,2,2) as 2024 production choices | Use T9 for the current disclosed layouts, and do not project them backward onto the 2016 system. [B3][T5][T8][T9] |
| Cell size | B2 says about 50 PB raw and a practical limit around 100 PB | T9 reports cells around 100 to 120 PB | Capacity grew; these are dated snapshots, not a contradiction. [B2][T9] |
| SMR date | T9 says work began in 2015 and also discusses a 2018 production deployment | B4 announced production deployment in 2018 | 2015 is development; 2018 is production rollout. [B4][T9] |
| AWS relationship | The project moved most user content off S3 | T4, T5 and T8 all describe continued AWS use | Magic Pocket was a hybrid repatriation, not a complete exit from AWS. [T4][T5][T8][F1] |

## Inputs for later work

- **Architecture (#8):** use T5 for the migration-era replicated layout, T8 for lifecycle and operational behavior, and T9 for the latest disclosed coding schemes and cell size.
- **Case background (#9):** use T4 and T5 for the migration deadline and mechanics; use T6 only for the stated strategic rationale, not for audited savings.
- **Design decisions (#11):** T2 supports guarded repair automation; T8 supports slow, reversible background migration; T9 supports regular verifier tests and failure drills.
- **Rust decision:** T3, T5, T7 and T9 independently trace the storage-node path from Go to Rust. The talks explain operational and runtime motivations more directly than the project brief should.
