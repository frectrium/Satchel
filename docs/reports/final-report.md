# Satchel final report

**Team:** Parshv Joshi (@frectrium), Guru Vyas (@VyasGuru), Jas Mehta (@JasMehta08). Edited by Jas.

**Release described:** v1.0 (#36), to be filled in once it is tagged.

Source IDs such as [B2] refer to the [Source Log](../sources/README.md).

## How this report is put together

Issue #28. Each owner writes their section in its own file in [`final/`](final/) and opens a PR for it, as #94 did for the Decision Log chapter. When the chapters are merged, the editor pastes them into this file in order, edits the whole report and exports it to PDF, as the Week 2 report was assembled (#61). Every claim about Dropbox cites its Source Log ID, and our own reasoning is marked *(our inference)*.

| # | Section | Owner | Issues | Chapter file | Status |
|---:|---|---|---|---|---|
| 1 | Case study: background, problem, solution and outcome | Jas | #9, #12, #13, from the Week 2 report (#18) | `final/case-study.md` | Not started |
| 2 | Architecture: Magic Pocket and Satchel | Parshv | #11 | `final/architecture.md` | Not started |
| 3 | Satchel implementation | Parshv | #19, #66 to #73 | `final/implementation.md` | Not started |
| 4 | Design Decision Log | Parshv | #27 | [`final/design-decisions.md`](final/design-decisions.md) | Merged (#94) |
| 5 | Testing approach and results | Jas | #31, #32 | `final/testing.md` | Not started |
| 6 | Experiments and results | Parshv, Guru, Jas (see below) | #22, #74 to #77 | `final/experiments.md` | Not started |
| 7 | Cost and repatriation analysis | Jas | #23 to #26, #52, #53 | `final/cost.md` | Not started |
| 8 | Conclusions, limitations and future work | All three, edited by Jas | none | `final/conclusions.md` | Not started |
| 9 | References | Jas | none | `final/references.md` | Not started |

Experiments by owner:

| Experiment | Owner | Issue |
|---|---|---|
| E1: latency against block size, Satchel and S3, with the load generator and S3 baseline (#22) | Parshv | #74 |
| E2: scaling with nodes, clients and data stored | Guru | #75 |
| E3 and E4: replication against erasure coding, and failure recovery | Jas | #76 |
| E5: Satchel's resource usage, as a check on the cost model | Parshv | #77 |

## 1. Case study

## 2. Architecture

## 3. Satchel implementation

## 4. Design Decision Log

## 5. Testing approach and results

## 6. Experiments and results

## 7. Cost and repatriation analysis

## 8. Conclusions, limitations and future work

## 9. References
