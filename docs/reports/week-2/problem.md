# The problem: why Dropbox built Magic Pocket

Dropbox did not begin as an all-public-cloud company. Its web and metadata services ran in its own datacenters, while Amazon S3 stored user file content. That split let the company grow quickly, but by 2016 Dropbox reported more than 500 million signups and 500 PB of user data, up from about 40 PB in 2012. At that scale, Dropbox said that controlling the storage stack could improve both product performance and unit economics. [B1]

This account uses the primary-source IDs in the [Source Log](../../sources/README.md). It separates what Dropbox disclosed from conclusions the public evidence cannot support.

## A scale problem, not a generic case against cloud

Magic Pocket became a major initiative in summer 2013, after a smaller prototype. Dropbox said that no open-source storage system had been proven reliable at its required scale, so using an existing self-hosted package was not an available shortcut. The system dark-launched in August 2014, first served selected user files exclusively in-house in February 2015, and served more than 90% of user data by October 2015. [B1]

The deadline was unusually sharp. Dropbox had a large AWS storage commitment that was due to expire, while its data continued to grow. It therefore had to build storage capacity, a multi-terabit network, and a safe migration path together. During the fastest phase, data moved at more than half a terabit per second; a later retrospective put the migration rate above 6 PB per day and the hardware build at roughly 30 to 40 racks of new capacity per day. [B1][T4][T5]

These facts matter because Dropbox did not claim that private infrastructure is always cheaper. Its engineers described cost as the leading reason **at Dropbox's scale**, while also saying that third-party infrastructure remained the right choice when its economics or geographic reach matched a workload. [T6]

## The cost pressure

Dropbox's public sources establish a unit-cost problem, but not a complete pre-migration AWS bill. The 2016 announcement says that a storage system tailored to Dropbox's block workload and scale could deliver better unit economics. A later engineering talk says the comparison included the whole hardware lifecycle, such as power, useful life, maintenance, and replacement timing, rather than purchase price alone. [B1][T6]

The S-1 supplies the audited outcome. In 2016, Dropbox spent $92.5 million less on third-party service providers, while depreciation, facilities, and support for its own colocation equipment increased by $53.0 million. The net reported decrease in infrastructure costs was therefore $39.5 million. Dropbox reported a further $35.1 million decrease in 2017. [F1, pp. 71-73]

Those figures need two qualifications. First, Dropbox grouped the migration with closing some long-inactive Basic accounts and later usage optimization under the name **Infrastructure Optimization**. The filing does not isolate Magic Pocket's build cost, exact AWS savings, or payback period. Second, the two annual infrastructure-cost decreases sum to $74.6 million, but that is our calculation, not a Dropbox-reported Magic Pocket savings figure. [F1, pp. 67, 71-73]

### Storage, requests, and egress

The evidence supports storage volume as a central scale pressure: S3 held Dropbox's file content, and the contemporary figure was 500 PB of user data. This characterization is our inference from the scale and cost evidence. The record also supports a large fall in third-party service-provider expense after most data moved to custom infrastructure. The filing does not, however, split the old bill into storage capacity, API requests, and network egress. [B1][F1, pp. 72-73]

Dropbox later described S3 API charges as a separate cost problem for internal objects. Rarely read objects still produced a large PUT bill, while cache misses incurred GET charges, making S3 an expensive default for that workload. This 2022 account shows why request volume can matter, but it should not be projected backward as a quantified share of the 2015 migration decision. [B10]

The primary sources reviewed here do not identify AWS egress charges as a measured driver of the decision. Network movement was a major operational challenge: Dropbox built a multi-terabit network and moved data at very high rates. That is evidence about migration capacity, not an itemized egress bill. Claiming that egress fees drove the move would go beyond the disclosed record. [T4][T5]

## Why cost was not the only reason

### End-to-end control and product performance

Dropbox called performance a product differentiator. Owning the storage stack let its teams customize hardware and software together rather than accept a general-purpose service interface. Dropbox grouped the benefits of that control as cost, performance, and agility. [B1][T6]

The reported performance outcome was material but specific. After the network and storage migration, a Dropbox talk reported median upload or download improvements of roughly 40% to 300%, depending on user location. That range is a network result, not proof that every request or every user became faster by the same amount. [T4]

### Hardware and software co-design

Direct control let Dropbox shape storage around immutable blocks and sequential writes. It later co-designed dense storage servers with the Magic Pocket team and adopted host-managed shingled magnetic recording drives for greater bit density and a better cost per gigabyte. The software wrote directly to raw zoned disks, and the storage-node implementation moved from Go to Rust to control memory allocation and garbage collection as disks became larger and more numerous. [B4][B9][T7]

This was more than replacing one vendor with commodity servers. Dropbox accepted responsibility for matching block layout, repair bandwidth, networking, disk behavior, and software runtime to the workload. Its stated advantage was the ability to change those layers together. [B4][T6]

### Vendor dependence without a complete cloud exit

An expiring AWS commitment imposed a hard date on the migration, which shows the planning effect of a large external storage contract. Dropbox's stated rationale, however, was control, performance, cost, and agility rather than a claim that AWS was unreliable. [T4][T6]

Dropbox deliberately kept a hybrid architecture. Its S-1 says more than 90% of user data was on custom infrastructure, while AWS stored the remainder and supported data delivery and regional localization. Engineering accounts also describe AWS use for European locality and compliance, and later S3 use for internal data such as build artifacts, test logs, and image caches. [F1, p. 120][T5][T8][B10]

The boundary followed workload fit. Magic Pocket concentrated on Dropbox's large, predictable user-content workload in North America; public cloud remained useful where geographic reach or workload economics made it the better choice. [T6][T8]

## The risks Dropbox took on

Leaving S3 for most user content transferred several risks to Dropbox:

| Risk | Evidence from the migration |
|---|---|
| Data loss or corruption | Durability was non-negotiable. Dropbox mirrored data during a six-month dark launch, kept extra backups, migrated users individually, verified the new copies, and only then deleted the S3 copies. A staging bug found after about 40 days reset the validation period. [B1][T5] |
| Deadline and capacity | The AWS commitment deadline coincided with rapid growth, forcing a large network and hardware build while the service stayed live. [T4][T5] |
| Duplicate cost during transition | Dropbox kept old and new copies at the same time, which its retrospective described as expensive. The filing says data was duplicated between third-party and custom infrastructure during the transition. [T5][F1, p. 67] |
| Capital and fixed commitments | The move replaced part of a service-provider bill with depreciation, facilities, support, equipment, and colocation obligations. The S-1 reports higher owned-infrastructure expense and substantial lease commitments, though it does not isolate a Magic Pocket total. [F1, pp. 72-73, 80] |
| Operational ownership | Dropbox became responsible for repair, verification, disk failures, capacity, and destructive maintenance. Its automation used availability checks, rate limits, staged confidence, and retained human approval for disk reformatting because a mistake could destroy the last good copy. [B2][B3][T2] |
| Technology execution | Dropbox had no proven open-source system at its scale and had to build the storage software, hardware integration, and migration tooling with a small team. [B1][B2] |

Dropbox reported reaching the October 2015 milestone without a major service disruption or loss of data. That success does not make the original risk small; it reflects the dark launch, duplicate storage, staged migration, continuous verification, and conservative automation used to control it. [B1][B3][T2][T5]

## The problem Magic Pocket had to solve

Dropbox needed a storage system whose unit economics improved at hundreds of petabytes, whose request path could be tuned for the product, and whose hardware could evolve with the workload. It also needed to move live user data under a contract deadline without weakening durability, while retaining AWS where public cloud still offered the better geographic or workload fit. [B1][T4][T5][T6]

Magic Pocket was therefore not simply an attempt to replace S3 with cheaper disks. It was a decision to own the complete storage lifecycle, including capital, operations, failure recovery, and verification, in exchange for cost, performance, and engineering control at Dropbox's scale. [T6][F1, pp. 67, 71-73]
