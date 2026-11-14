#!/usr/bin/env python3
"""Hardware part of the on-prem estimate (#25): servers, drives and switches
to own the case workload (#15), bought year by year over the horizon.

    python3 docs/cost/on-prem/hardware.py

Writes hardware-inputs.csv and hardware.csv next to this file. Standard
library only. The sizing rules match the sizing model in satchel-cost (#53):
whole servers, at least 9 per region for RS(6,3), whole racks.
"""
import csv
import math
from pathlib import Path

# (name, value, unit, source). "our inference" marks values we chose.
INPUTS = [
    ("start_pb", 500, "PB stored at the baseline", "[B1], workload model #15"),
    ("growth", 0.20, "per year", "[T5], workload model #15"),
    ("horizon_years", 5, "years", "workload model #15; server life 5 to 6 years [H13]"),
    ("raw_overhead", 3.0, "raw bytes per stored byte", "two regions, each RS(6,3) [B2][T8][T9], workload model #15"),
    ("regions", 2, "regions", "[B2][B5]"),
    ("drive_tb", 32, "TB per drive", "drives up to 32 TB [B14]"),
    ("drives_per_server", 100, "drives", "more than 100 drives per server, so a lower bound [B9]"),
    ("servers_per_rack", 8, "servers", "8 chassis per rack [B9]"),
    ("min_servers_per_region", 9, "servers", "RS(6,3) puts 9 fragments on 9 servers (our inference)"),
    ("server_usd_per_tb_base", 35.0, "USD per raw TB, server and drives", "Storage Pod 6.0, about $35,000 per PB [H3]"),
    ("server_usd_per_tb_low", 32.0, "USD per raw TB, server and drives", "Storage Pod 6.0 with 12 TB drives, $0.032 per GB [H3]"),
    ("drive_only_usd_per_tb_aug2026_low", 27.92, "USD per TB, drives only", "Seagate Exos SATA, Aug 2026 [H2]"),
    ("drive_only_usd_per_tb_aug2026_high", 44.17, "USD per TB, drives only", "WD Gold, Aug 2026 [H2]"),
    ("tor_switches_per_rack", 1, "switches", "one 32-port top-of-rack switch: 2 x 100G per server down, 16 up (our inference)"),
    ("rack_uplinks", 16, "100G ports", "non-blocking leaf-spine (our inference)"),
    ("switch_ports", 32, "100G ports", "[H14]"),
    ("switch_usd", 7432.57, "USD per switch", "Edgecore 7716-32X [H14]. Optics and cables not included"),
]
I = {name: value for name, value, _, _ in INPUTS}
SWEEP_PB = [0.1, 1, 10, 100, 500]


def layout(stored_pb):
    """Servers, racks and switches across all regions for stored_pb."""
    regions = I["regions"]
    raw_pb = stored_pb * I["raw_overhead"]
    server_pb = I["drive_tb"] * I["drives_per_server"] / 1000
    per_region = max(math.ceil(raw_pb / regions / server_pb), I["min_servers_per_region"])
    racks = math.ceil(per_region / I["servers_per_rack"])
    spines = math.ceil(racks * I["rack_uplinks"] / I["switch_ports"])
    return {
        "raw_pb": raw_pb,
        "servers": per_region * regions,
        "drives": per_region * regions * I["drives_per_server"],
        "racks": racks * regions,
        "switches": (racks * I["tor_switches_per_rack"] + spines) * regions,
        "installed_pb": per_region * regions * server_pb,
    }


def rows(start_pb):
    """Year by year. Capacity for the end of each year is bought at its start."""
    have = {"servers": 0, "switches": 0}
    server_tb = I["drive_tb"] * I["drives_per_server"]
    out = []
    for year in range(1, I["horizon_years"] + 1):
        stored_end = start_pb * (1 + I["growth"]) ** year
        l = layout(stored_end)
        bought = l["servers"] - have["servers"]
        switches_bought = l["switches"] - have["switches"]
        have = {"servers": l["servers"], "switches": l["switches"]}
        out.append({
            "start_pb": start_pb,
            "year": year,
            "stored_end_pb": round(stored_end, 3),
            "raw_needed_pb": round(l["raw_pb"], 3),
            "installed_raw_pb": round(l["installed_pb"], 3),
            "servers": l["servers"],
            "drives": l["drives"],
            "racks": l["racks"],
            "switches": l["switches"],
            "servers_bought": bought,
            "switches_bought": switches_bought,
            "server_capex_usd_low": round(bought * server_tb * I["server_usd_per_tb_low"]),
            "server_capex_usd_base": round(bought * server_tb * I["server_usd_per_tb_base"]),
            "network_capex_usd": round(switches_bought * I["switch_usd"]),
        })
    return out


def main():
    here = Path(__file__).parent
    with open(here / "hardware-inputs.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["input", "value", "unit", "source"])
        w.writerows(INPUTS)
    all_rows = [r for pb in SWEEP_PB for r in rows(pb)]
    with open(here / "hardware.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(all_rows[0]))
        w.writeheader()
        w.writerows(all_rows)
    for pb in SWEEP_PB:
        r = rows(pb)
        total = sum(x["server_capex_usd_base"] + x["network_capex_usd"] for x in r)
        print(f"{pb:>6} PB: {r[-1]['servers']:>5} servers, {r[-1]['racks']:>4} racks, "
              f"{r[-1]['switches']:>4} switches, 5-year capex ${total:,.0f}")


if __name__ == "__main__":
    main()
