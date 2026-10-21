# Filing evidence: Magic Pocket cost and outcomes

Research notes for [#4](https://github.com/frectrium/Satchel/issues/4). Figures are in USD millions and are copied from Dropbox's SEC filings unless marked as a calculation or interpretation.

- **Sources:** `F1` and `F2` in the [Source Log](../sources/README.md#f-filings).
- **Accessed:** 2026-10-21.
- **Rule used here:** Dropbox's term **Infrastructure Optimization** includes the S3 migration, management of long-inactive Basic accounts and later usage optimization. It is broader than Magic Pocket alone. [F1, p. 67]

## F1: Form S-1 filed 2018-02-23

### What Dropbox reported

Dropbox said it moved most user data from third-party infrastructure to lower-cost custom infrastructure in colocation facilities that it leased and operated. During the transition it duplicated user data between the two systems, then “completed the migration in the fourth quarter of 2016.” [F1, p. 67]

The same section says the company also closed some long-inactive Basic accounts and made additional usage optimizations in 2017. Dropbox grouped all of those measures under **Infrastructure Optimization** and said they reduced unit costs and helped limit capital expenditure and depreciation. Paying-user growth happened at the same time. [F1, p. 67]

### Revenue, cost of revenue and gross margin

| Year | Revenue | Cost of revenue | Gross profit | Gross margin | Infrastructure-cost decrease |
|---:|---:|---:|---:|---:|---:|
| 2015 | $603.8 | $407.4 | $196.4 | 33% | not reported |
| 2016 | $844.8 | $390.6 | $454.2 | 54% | $39.5 |
| 2017 | $1,106.8 | $368.9 | $737.9 | 67% | $35.1 |

Source: S-1 results-of-operations tables. [F1, pp. 71-73]

- **2016:** cost of revenue fell $16.8M even as revenue grew 40%. Dropbox reported a $39.5M net decrease in infrastructure costs: a $92.5M decline in third-party service-provider expense, partly offset by $53.0M more depreciation, facilities and support expense for equipment in its own colocation facilities. [F1, pp. 72-73]
- **2017:** cost of revenue fell another $21.7M as revenue grew 31%. Dropbox attributed the decrease primarily to a $35.1M infrastructure-cost reduction and linked the gross-margin increase to both revenue growth and Infrastructure Optimization. [F1, pp. 71-72]

For later citation, the filing's exact amount phrases are “$39.5 million in infrastructure costs” for 2016 and “$35.1 million in our infrastructure costs” for 2017. [F1, pp. 71-73]

The gross-margin change from 33% in 2015 to 67% in 2017 is reported. Treating the full 34 percentage-point gain as a Magic Pocket result would not be supported: revenue and paying users grew, and the optimization program included work beyond the migration. [F1, pp. 67, 71-73]

### Cash flow and capital expenditure

| Year | Cash from operations | Capital expenditure | Free cash flow |
|---:|---:|---:|---:|
| 2015 | $14.8 | $78.7 | -$63.9 |
| 2016 | $252.6 | $115.2 | $137.4 |
| 2017 | $330.3 | $25.3 | $305.0 |

Dropbox defines free cash flow as operating cash flow less capital expenditure. It said the increase reflected higher subscription cash receipts and, in 2017, lower capital expenditure related to office and datacenter build-outs. These company-wide figures are not a Magic Pocket return-on-investment calculation. [F1, pp. 66-67]

### Contractual commitments

As of 2017-12-31, Dropbox reported these future cash commitments: [F1, p. 80]

| Commitment | Total |
|---|---:|
| Operating leases | $1,349.9 |
| Capital leases | $182.0 |
| Other commitments | $155.1 |

Operating leases covered both office space and datacenter operations; capital leases covered datacenter equipment. The filing does not split the operating-lease total between offices and storage, so it cannot be used as the cost of Magic Pocket. [F1, p. 80]

### What remained on AWS

The filing says Dropbox stored more than 90% of users' data on its custom infrastructure and used Amazon Web Services for the remainder and for data delivery. AWS datacenters in the United States and Europe also supported regional data-localization requirements. [F1, p. 120]

The reported share implies less than 10% remained outside the custom infrastructure at that date, but the filing gives neither an exact number of petabytes nor the AWS spend for that remainder. *(our inference)* [F1, p. 120]

### Reported facts versus calculations

| Statement | Classification | Safe wording |
|---|---|---|
| Infrastructure costs decreased $39.5M in 2016 and $35.1M in 2017 | Reported | Use the two annual figures and the filing's stated drivers. [F1, pp. 71-73] |
| The two decreases total $74.6M | Our calculation | Say “the reported annual decreases sum to $74.6M”; do not call it Dropbox's reported Magic Pocket savings. [F1, pp. 71-73] |
| Gross margin rose from 33% to 67% | Reported | Pair it with revenue growth and the broader optimization program. [F1, pp. 67, 71-73] |
| Magic Pocket cost $X or paid back in Y months | Not disclosed | Do not make this claim from the S-1. [F1] |
| Dropbox completely left AWS | Contradicted | The filing says AWS stored the remainder and supported delivery and localization. [F1, p. 120] |

## F2: 2018 Form 10-K follow-up

The next annual filing provides a useful boundary on the trend, but not a new migration-only number:

| 2018 result | Reported value |
|---|---:|
| Revenue | $1,391.7 |
| Cost of revenue | $394.7 |
| Gross profit | $997.0 |
| Gross margin | 72% |
| Capital expenditure | $63.0 |
| Free cash flow | $362.4 |

Source: 2018 10-K, pp. 44 and 49. [F2]

Infrastructure costs decreased $23.3M in 2018 through continued usage optimization, partly offsetting other increases in cost of revenue. That figure included a $16.1M depreciation reduction after Dropbox extended the estimated useful life of some infrastructure equipment from three to four years. It should not be added to the S-1 figures and labeled migration savings. [F2, pp. 45, 49]

Gross margin reached 72%, but Dropbox attributed the increase to 26% revenue growth and lower infrastructure costs, with stock-based compensation partly offsetting those effects. Capital expenditure rose to $63.0M, primarily for office and datacenter build-outs, which also shows why a single year's company-wide capital spending is not the cost of Magic Pocket. [F2, pp. 44, 49]

## Conclusions for the case study

1. The filing supports a real unit-cost and infrastructure-cost improvement after Dropbox moved most user data to its own system. [F1, pp. 67, 71-73]
2. It does not isolate the migration from account cleanup, usage optimization, revenue growth or later accounting-estimate changes. [F1, p. 67][F2, pp. 45, 49]
3. It discloses neither the complete Magic Pocket build cost nor a migration payback period. [F1]
4. The outcome was hybrid: Dropbox kept a smaller AWS footprint for storage, delivery and regional requirements. [F1, p. 120]
