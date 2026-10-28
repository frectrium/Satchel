# Satchel Docs

| Path | What lives there |
|---|---|
| [`SATCHEL_PROJECT.md`](SATCHEL_PROJECT.md) | Project brief: scope, objectives, test environment, week-by-week plan |
| [`sources/`](sources/README.md) | Source log. Every source we cite, with a stable ID |
| [`research/`](research/) | Research notes, one file per topic (architecture, cost, outcome, pricing) |
| `reports/` | Weekly deliverables (Week 1 summary, Week 2 report, final report) |
| [`design/`](design/) | Architecture diagrams, exercise plan, test strategy and the Design Decision Log |
| `experiments/` | How to run the load generator and reproduce each experiment |
| `cost/` | AWS, Azure and on-prem cost models and the comparison |

Folders without a link above don't exist yet. Create them with your first file in them.

## Conventions

- Write in Markdown so everything diffs and reviews in PRs.
- Cite sources by Source Log ID, e.g. *"Magic Pocket stores blocks of up to 4 MB [B2]"*.
- Separate what a source says from our interpretation. Mark inferences with *"(our inference)"*.
- Put diagrams in `design/` as Mermaid in Markdown, or as an exported `.svg` next to its source file (`.excalidraw`, `.drawio`).
