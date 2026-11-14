#!/usr/bin/env python3
"""Plots satchel-bench results: latency percentiles against block size.

    scripts/plot-results.py docs/results/e1/*.json -o docs/results/e1/latency.png

Each line is one target (satchel or s3) and operation (put or get). Needs
matplotlib (pip install matplotlib).
"""
import argparse
import json
from collections import defaultdict

import matplotlib.pyplot as plt


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("results", nargs="+", help="satchel-bench JSON files")
    parser.add_argument("-o", "--out", default="latency.png")
    parser.add_argument("--percentiles", default="p50,p99", help="any of p50,p90,p99,p999")
    args = parser.parse_args()

    # (target, op) -> [(block_size, op stats)]
    series = defaultdict(list)
    for path in args.results:
        with open(path) as f:
            report = json.load(f)
        size = report["config"]["block_size"]
        for op, stats in report["ops"].items():
            series[(report["target"], op)].append((size, stats))

    fig, ax = plt.subplots(figsize=(8, 5))
    for (target, op), points in sorted(series.items()):
        points.sort(key=lambda p: p[0])
        sizes = [size / 1024 for size, _ in points]
        for pct in args.percentiles.split(","):
            ax.plot(sizes, [s[f"{pct}_ms"] for _, s in points], marker="o",
                    label=f"{target} {op} {pct}")
    ax.set_xscale("log", base=2)
    ax.set_yscale("log")
    ax.set_xlabel("Block size (KiB)")
    ax.set_ylabel("Latency (ms)")
    ax.grid(True, which="both", alpha=0.3)
    ax.legend()
    fig.tight_layout()
    fig.savefig(args.out, dpi=150)
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
