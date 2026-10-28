# Team roles

Issue #16. Who owns what in Satchel. The work is split equally across the three of us. Ownership follows the issue assignments on the [project board](https://github.com/users/frectrium/projects/1), so when an issue changes owner, this doc changes too.

## 1. Members and areas

| Member | Main area | Also owns |
|---|---|---|
| Parshv Joshi (@frectrium) | Architecture and storage design | Repo and board (owner of `main` ruleset), exercise plan, Design Decision Log, observability, load generator and S3 baseline |
| Guru Vyas (@VyasGuru) | Case analysis and cloud infrastructure | Filings and talks research, problem and solution sections, AWS account, Terraform, Docker image, deployments, release |
| Jas Mehta (@JasMehta08) | Cost and economics, testing | Pricing and on-prem references, workload model, AWS and Azure calculator models, break-even comparison, test strategy and integration test harness |

## 2. Issues by owner

| Week | Parshv (@frectrium) | Vyas (@VyasGuru) | Jas (@JasMehta08) |
|---|---|---|---|
| 1 | #2 Tech blog sources, #8 Architecture notes, #30 Workflow docs, editor of #10 | #3 Talks, #4 S-1 data, #9 Case background | #5 Media, #6 Cloud pricing, #7 On-prem prices |
| 2 | #11 Architecture diagram, #14 Exercise plan, #17 Design Decision Log, #19 Week 3 issues | #12 Problem, #13 Solution, #33 AWS groundwork, editor of #18 | #15 Workload model, #31 Test strategy, #32 Integration harness, editor of #16 |
| 3 | #21 Observability, #22 Load generator and S3 baseline | #20 Terraform, #34 Docker image, #35 First AWS deployment | #23 AWS calculator, #24 Azure calculator, #26 Break-even |
| 4 | #27 Fold Decision Log into the report | #36 Release v1.0 | |
| Count | 10 issues + 1 editor role | 10 issues + 1 editor role | 9 issues + 1 editor role |

**Shared by all three:**
- #1 Local setup
- #10 Week 1 summary
- #16 Team roles
- #18 Week 2 report
- #25 On-prem estimate
- #28 Final report
- #29 Viva
- #37 Week 3 results

The build issues created in #19 will be added to the table above when they exist.

## 3. Report sections

| Section | Owner | Issue |
|---|---|---|
| Introduction and case background | Vyas | #9 |
| Problem: why Dropbox left S3 | Vyas | #12 |
| Architecture diagram and explanation | Parshv | #11 |
| Solution: how Magic Pocket solves it | Vyas | #13 |
| Exercise plan, including erasure coding | Parshv | #14 |
| Workload model | Jas | #15 |
| Test strategy summary | Jas | #31 |
| Team roles | Jas (editor), all | #16 |
| Week 3: cost results (AWS, Azure, on-prem, break-even) | Jas | #23, #24, #26, with #25 shared |
| Week 3: latency and scaling results | Parshv | #21, #22 |
| Week 3: deployment | Vyas | #20, #35 |
| References (from the Source Log) | Vyas, as report editor | #18 |

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
| Terraform and deployment | n/a (Dropbox ran its own data centers) | Vyas (#20, #35) |
| Metrics and dashboards | n/a | Parshv (#21) |

## 5. How we work together

- **Reviews:** every PR needs one approval. CODEOWNERS requests all three of us.
- **Report sign-off:** each member approves the weekly report PR. The editor (Parshv for Week 1, Vyas for Week 2) merges it.
- **This doc:** each member approves its PR.
- **Blockers:** an owner who is blocked says so on the issue and adds the `blocked` label.
- **Workflow:** branching, commits and the definition of done are in [CONTRIBUTING.md](../CONTRIBUTING.md).
