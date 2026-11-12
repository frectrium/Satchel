# Running the experiments

The experiments in the exercise plan (#14, section 4) run on the local Compose cluster with `satchel-bench`, the load generator (#22). The only run on AWS is E1's S3 comparison, from the demo cluster before the viva (#63).

## The load generator

```bash
cargo run --release -p satchel-bench -- \
  --block-size 2MiB --mix 2:1 --clients 16 \
  --warmup 30 --duration 300 --max-ops 10000 \
  --out docs/results/e1/satchel-2MiB-c16-r1.json
```

| Option | Default | Meaning |
|---|---|---|
| `--target` | `satchel` | `satchel` or `s3` |
| `--url` | `http://localhost:8080` | `satchel-api` |
| `--block-size` | `2MiB` | Size of every block, `4KiB` to `4MiB` |
| `--mix` | `2:1` | Gets to puts. The workload model has egress of twice the ingest (#15) |
| `--clients` | 16 | Concurrent clients |
| `--warmup` | 30 | Seconds run first and not measured |
| `--duration`, `--max-ops` | 300, 10000 | Measuring stops at whichever comes first |
| `--prefill` | `0` | Data stored before the run and not measured, for E2, for example `10GiB` |
| `--seed` | 1 | Block contents and read order come from the seed, so a run can be repeated |
| `--out` | stdout | Where to write the JSON result |

Every put sends a new block keyed by its SHA-256. Every get reads a block stored earlier and checks its hash, so wrong data counts as an error.

The result has the configuration, the measured window, and for each operation the sample count, errors, p50, p90, p99 and p99.9 latency, mean and max, ops/s and MiB/s. The percentiles are exact (nearest rank over every sample), unlike the dashboard's histogram estimates (#21).

## Repeating a run

- Run each configuration 3 times with seeds 1, 2 and 3, and name files `<target>-<size>-c<clients>-r<seed>.json`.
- Record the commit (`git rev-parse HEAD`) and the cluster setup in the experiment's notes.
- Keep results in `docs/results/<experiment>/`.

## Plotting

```bash
pip install matplotlib
scripts/plot-results.py docs/results/e1/*.json -o docs/results/e1/latency.png
```

## The S3 run

Only from the AWS demo cluster, in ap-south-1, so S3's numbers don't include internet latency. Build with the S3 backend and take credentials from the AWS CLI profile:

```bash
cargo build --release -p satchel-bench --features s3
eval "$(aws configure export-credentials --profile satchel --format env)"
target/release/satchel-bench --target s3 --s3-bucket <bucket> --block-size 2MiB \
  --out docs/results/e1/s3-2MiB-c16-r1.json
```

Never put the credentials in a file or on the command line. The bucket is created and destroyed with the demo cluster (#20).
