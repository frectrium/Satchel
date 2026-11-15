# Cost engine output

Written by the cost engine (#52). Don't edit these files by hand. Regenerate them after a price or workload change:

```bash
cargo run -p satchel-cost -- --prices crates/satchel-cost/prices --out docs/cost/output
```

| File | One row per | Columns |
|---|---|---|
| `yearly.csv` | scenario, provider, year and cost component | `usd`. Components that aren't added to the total (cash capex, depreciation, server and node counts) are in the same file |
| `totals.csv` | scenario and provider | `total_usd` over the 5-year horizon |
| `sweep.csv` | scenario, starting size and provider | `total_usd` at 0.1, 1, 10, 100 and 500 PB |
| `curve.csv` | scenario, starting size and provider | `total_usd` at 33 sizes from 0.1 PB to 1 EB, for the break-even chart (#26) |
| `sensitivity.csv` | scenario | AWS, Azure and on-prem totals over 5 years, and the break-even size against AWS and Azure |
| `flagged-inputs.csv` | input | Inputs that are proposed, assumed or not yet sourced, and which value the engine uses |
| `break-even.csv` | scenario and cloud option | `break_even_start_pb`: the smallest starting size at which owning costs no more over 5 years |

Providers: `aws` and `azure` (cold data on S3 Standard-IA and Azure Cool ZRS), `aws_glacier_ir` and `azure_cold` (the cheaper cold tiers), `on_prem` and `satchel_ec2`. `cargo run -p satchel-cost -- --list` describes each scenario.
