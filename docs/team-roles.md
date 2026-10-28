# Team roles

Issue #16. Who owns what in Satchel. The work is split equally across the three of us. Ownership follows the issue assignments on the [project board](https://github.com/users/frectrium/projects/1), so when an issue changes owner, this doc changes too.

## 1. Members and areas

| Member | Main area | Also owns |
|---|---|---|
| Parshv Joshi (@frectrium) | Architecture and storage design | Repo and board (owner of `main` ruleset), exercise plan, Design Decision Log, observability, load generator and S3 baseline. Editor of the Week 1 summary, viva coordinator |
| Guru Vyas (@VyasGuru) | Case analysis and cloud infrastructure | Filings and talks research, problem and solution sections, AWS account, Terraform, Docker image, deployments, release. Editor of the Week 2 report and the Week 3 add-ons |
| Jas Mehta (@JasMehta08) | Cost and economics, testing | Pricing and on-prem references, workload model, AWS and Azure calculator models, break-even comparison, test strategy and integration test harness. Editor of this doc and the final report |

## 2. Issues by owner

| Week | Parshv (@frectrium) | Guru (@VyasGuru) | Jas (@JasMehta08) |
|---|---|---|---|
| 1 | #2 Tech blog sources, #8 Architecture notes, #30 Workflow docs, editor of #10 | #3 Talks, #4 S-1 data, #9 Case background | #5 Media, #6 Cloud pricing, #7 On-prem prices |
| 2 | #11 Architecture diagram, #14 Exercise plan, #17 Design Decision Log, #19 Week 3 issues | #12 Problem, #13 Solution, #33 AWS groundwork, editor of #18 | #15 Workload model, #31 Test strategy, #32 Integration harness, editor of #16 |
| 3 | #21 Observability, #22 Load generator and S3 baseline | #20 Terraform, #34 Docker image, #35 First AWS deployment, editor of #37 | #23 AWS calculator, #24 Azure calculator, #26 Break-even |
| 4 | #27 Fold Decision Log into the report | #36 Release v1.0 | Editor of #28 |
| 5 | Coordinator of #29 | | |
| Count | 10 issues + 2 lead roles | 10 issues + 2 lead roles | 9 issues + 2 lead roles |

**Shared by all three:**
- #1 Local setup
- #10 Week 1 summary
- #16 Team roles
- #18 Week 2 report
- #25 On-prem estimate, one part each: hardware (Parshv), power, space and network (Guru), staff and amortization (Jas)
- #28 Final report
- #29 Viva, with equal speaking time for all three
- #37 Week 3 results

The build issues created in #19 will be added to the table above when they exist.

## 3. Report sections

Section owners are the owners of the linked issues, as #18 and #37 set out. A section with no linked issue is marked **to be agreed**.

### Week 2 report (#18, edited by Guru)

| Section | Owner | Issue |
|---|---|---|
| Introduction and case background | Guru | #9 |
| Problem: why Dropbox left S3 | Guru | #12 |
| Architecture diagram and explanation | Parshv | #11 |
| Solution: how Magic Pocket solves it | Guru | #13 |
| Exercise plan, including erasure coding | Parshv | #14 |
| Workload model | Jas | #15 |
| Test strategy summary | Jas | #31 |
| Team roles | Jas (editor), all | #16 |
| References (from the Source Log) | To be agreed | none |

### Week 3 add-ons (#37, edited by Guru)

| Section | Owner | Issue |
|---|---|---|
| What we built | Owners of the build issues | #19 |
| Test results | Jas | #31, #32 |
| Experiment results | Parshv, plus owners of the experiment issues | #22, #19 |
| Cost results: AWS, Azure, on-prem and break-even | Jas, with #25 shared | #23, #24, #26 |
| Changes from the exercise plan | To be agreed | none |

### Final report (#28, edited by Jas)

| Section | Owner | Issue |
|---|---|---|
| Case study: background, problem, solution and outcome | Guru | #9, #12, #13 |
| Architecture: Magic Pocket and Satchel | Parshv | #11 |
| Satchel implementation | Owners of the build issues | #19 |
| Design Decision Log chapter | Parshv | #27 |
| Testing approach and results | Jas | #31, #32 |
| Experiments and results | Parshv, plus owners of the experiment issues | #22, #19 |
| Cost and repatriation analysis | Jas | #26 |
| Conclusions, limitations and future work | To be agreed | none |
| References | To be agreed | none |

## 4. Satchel components

The build issues come from the exercise plan (#14) through #19, so component owners are **to be agreed** when #19 creates them. This table is updated then.

| Component | Magic Pocket counterpart | Owner |
|---|---|---|
| `satchel-api` | Frontends | To be agreed in #19 |
| `satchel-index` | Block Index | To be agreed in #19 |
| `satchel-node` | OSD | To be agreed in #19 |
| Replication | Cross-zone replication | To be agreed in #19 |
| Erasure coding | Erasure-coded volumes | To be agreed in #19 |
| `satchel-verifier` | Pocket Watch | To be agreed in #19 |
| Integration test harness | Release testing with failure injection [B3] | Jas (#32) |
| Terraform, Docker image and deployment | n/a (Dropbox ran its own data centers) | Guru (#20, #34, #35) |
| Metrics and dashboards | n/a | Parshv (#21) |
| Load generator and S3 baseline | n/a | Parshv (#22) |

## 5. How we work together

- **Reviews:** every PR needs one approval. CODEOWNERS requests all three of us.
- **Reports:** each member reviews and approves every report PR (#10, #18, #37, #28). The editor in section 2 puts the report together.
- **This doc:** each member approves its PR.
- **Blockers:** an owner who is blocked says so on the issue and adds the `blocked` label.
- **Workflow:** branching, commits and the definition of done are in [CONTRIBUTING.md](../CONTRIBUTING.md).
