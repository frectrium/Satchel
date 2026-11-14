#!/usr/bin/env python3
"""Draws the charts for the cost comparison (#26) from the cost engine's CSV
output (#52).

    cargo run -p satchel-cost -- --scenario all --format csv
    scripts/plot-cost.py                      # reads docs/cost/output/

Writes break-even.png, breakdown.png and sensitivity.png to docs/cost/charts/.
Needs matplotlib (pip install matplotlib).
"""
import argparse
import csv
from collections import defaultdict
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

LINES = [
    ("aws", "AWS S3 (Standard + Standard-IA)", "#1f77b4", "-"),
    ("azure", "Azure Blob (Hot + Cool ZRS)", "#ff7f0e", "--"),
    ("aws_glacier_ir", "AWS S3 (Standard + Glacier IR)", "#1f77b4", ":"),
    ("azure_cold", "Azure Blob (Hot + Cold ZRS)", "#ff7f0e", ":"),
    ("on_prem", "On-prem (Magic Pocket layout)", "#2ca02c", "-"),
]


def read(path):
    with open(path, newline="") as f:
        return list(csv.DictReader(f))


def break_even_chart(out, curve, breaks):
    fig, ax = plt.subplots(figsize=(8, 5))
    for key, label, colour, style in LINES:
        pts = sorted((float(r["start_pb"]), float(r["total_usd"]) / 1e6)
                     for r in curve if r["scenario"] == "base" and r["provider"] == key)
        ax.plot([p for p, _ in pts], [c for _, c in pts], style, color=colour, label=label, linewidth=2)
    for r in breaks:
        if r["scenario"] == "base" and r["cloud"] in ("aws", "aws_glacier_ir") and r["break_even_start_pb"] != "never":
            pb = float(r["break_even_start_pb"])
            ax.axvline(pb, color="grey", linewidth=0.8, linestyle="--")
            ax.annotate(f"{pb:.1f} PB", (pb, 0.98), xycoords=("data", "axes fraction"),
                        rotation=90, va="top", ha="right", fontsize=8, color="grey")
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("Data stored at the start (PB, log scale)")
    ax.set_ylabel("Total cost over 5 years (millions of USD, log scale)")
    ax.set_title("Owning beats renting above about 15 PB (base case)")
    ax.grid(True, which="both", alpha=0.3)
    ax.legend(fontsize=8)
    fig.tight_layout()
    fig.savefig(out / "break-even.png", dpi=150)
    plt.close(fig)


def breakdown_chart(out, yearly):
    groups = {
        "aws": {"Storage": ["storage_hot", "storage_cold"], "Requests": ["requests_put", "requests_get", "transitions"],
                "Retrieval": ["retrieval"], "Egress": ["egress"]},
        "azure": {"Storage": ["storage_hot", "storage_cold"], "Requests": ["requests_put", "requests_get", "transitions"],
                  "Retrieval": ["retrieval"], "Egress": ["egress"]},
        "on_prem": {"Hardware": ["hardware"], "Power": ["power"], "Colocation": ["colocation"],
                    "Transit": ["transit"], "Staff": ["staff"]},
    }
    totals = defaultdict(float)
    for r in yearly:
        if r["scenario"] == "base":
            totals[(r["provider"], r["component"])] += float(r["usd"])
    names = {"aws": "AWS S3", "azure": "Azure Blob", "on_prem": "On-prem"}
    colours = {"Storage": "#1f77b4", "Requests": "#9467bd", "Retrieval": "#8c564b", "Egress": "#d62728",
               "Hardware": "#2ca02c", "Power": "#bcbd22", "Colocation": "#17becf", "Transit": "#7f7f7f",
               "Staff": "#e377c2"}
    fig, ax = plt.subplots(figsize=(8, 5))
    seen = set()
    for x, (provider, parts) in enumerate(groups.items()):
        bottom = 0.0
        for part, components in parts.items():
            value = sum(totals[(provider, c)] for c in components) / 1e6
            ax.bar(x, value, bottom=bottom, color=colours[part], label=None if part in seen else part)
            seen.add(part)
            bottom += value
        ax.text(x, bottom, f"${bottom:,.0f}M", ha="center", va="bottom", fontsize=9)
    ax.set_xticks(range(len(groups)))
    ax.set_xticklabels([names[p] for p in groups])
    ax.set_ylabel("Total over 5 years (millions of USD)")
    ax.set_title("500 PB base case, 5 years")
    ax.legend(fontsize=8)
    fig.tight_layout()
    fig.savefig(out / "breakdown.png", dpi=150)
    plt.close(fig)


def sensitivity_chart(out, breaks, labels):
    rows = [(r["scenario"], r["break_even_start_pb"]) for r in breaks if r["cloud"] == "aws"]
    base = next(float(v) for s, v in rows if s == "base")
    rows = [(s, float(v)) for s, v in rows if s != "base" and v != "never"]
    rows.sort(key=lambda r: abs(r[1] - base))
    fig, ax = plt.subplots(figsize=(8, 6))
    ys = range(len(rows))
    ax.barh(ys, [v - base for _, v in rows], left=base,
            color=["#d62728" if v > base else "#2ca02c" for _, v in rows])
    ax.axvline(base, color="black", linewidth=1)
    ax.set_yticks(list(ys))
    ax.set_yticklabels([labels.get(s, s) for s, _ in rows], fontsize=8)
    for y, (_, v) in zip(ys, rows):
        ax.text(v, y, f" {v:.1f}", va="center", fontsize=8)
    ax.set_xlabel("Break-even starting size against AWS S3 (PB). Base case: %.1f PB" % base)
    ax.set_title("What moves the break-even point")
    fig.tight_layout()
    fig.savefig(out / "sensitivity.png", dpi=150)
    plt.close(fig)


LABELS = {
    "growth_minus_50": "Growth 10% a year",
    "growth_plus_50": "Growth 30% a year",
    "growth_high": "Growth 41% a year",
    "egress_minus_50": "Egress 1x ingest",
    "egress_plus_50": "Egress 3x ingest",
    "egress_5x": "Egress 5x ingest",
    "block_1mb": "1 MB blocks",
    "block_4mb": "4 MB blocks",
    "two_regions": "Second cloud region",
    "server_low": "Server $32 per TB",
    "server_double": "Server price doubled",
    "life_4": "4-year hardware life",
    "life_6": "6-year hardware life",
    "capital_low": "5.3% cost of capital",
    "capital_high": "10.66% cost of capital",
    "power_idle": "Drives at idle power",
    "power_rack_budget": "Racks at full power budget",
    "staff_low": "Smaller team",
    "cold_tier_on_prem": "Cold tier on-prem (2.25x)",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--input", default="docs/cost/output", help="satchel-cost CSV output")
    parser.add_argument("--out", default="docs/cost/charts")
    args = parser.parse_args()
    src, out = Path(args.input), Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    breaks = read(src / "break-even.csv")
    break_even_chart(out, read(src / "curve.csv"), breaks)
    breakdown_chart(out, read(src / "yearly.csv"))
    sensitivity_chart(out, breaks, LABELS)
    print(f"wrote {out}/break-even.png, breakdown.png, sensitivity.png")


if __name__ == "__main__":
    main()
