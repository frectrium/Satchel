#!/usr/bin/env python3
"""Staff and amortization part of the on-prem estimate (#25): the people who
run the storage, and what the hardware from hardware.py costs per year once
it is spread over its life with a cost of capital.

    python3 docs/cost/on-prem/hardware.py             # first, if inputs changed
    python3 docs/cost/on-prem/staff-amortization.py

Reads hardware.csv (the hardware part) and writes staff-amortization-inputs.csv
and staff-amortization.csv next to this file. Standard library only.
"""
import csv
from pathlib import Path

# (name, value, unit, source). "our inference" and "our calc" mark values we
# chose or worked out.
INPUTS = [
    ("software_engineers", 9, "FTE", "about 8 to 10 engineers operated the storage software in 2024 [T9]. Midpoint"),
    ("software_engineers_low", 8, "FTE", "[T9]"),
    ("software_engineers_high", 10, "FTE", "[T9]"),
    ("engineer_salary_usd", 135980, "USD per year, base pay", "US median, software developers, May 2025 [H16]"),
    ("sites", 2, "sites", "two regions, as in the hardware part [B2][B5]"),
    ("technicians_on_site", 2, "people on site at all times, per site", "one to two qualified operators on site 24x7 [H17]. Upper end"),
    ("technicians_on_site_low", 1, "people on site at all times, per site", "[H17]"),
    ("hours_per_fte", 2080, "hours per year", "40 hours x 52 weeks (our calc). Ignores leave, so it understates headcount"),
    ("technician_salary_usd", 99130, "USD per year, base pay", "US median, network and computer systems administrators, May 2025 [H11]"),
    ("wage_share_of_cost", 0.700, "wages / total employer cost", "private industry, June 2026 [H15]"),
    ("life_years", 5, "years", "datacenter equipment 3 to 5 years, some infrastructure moved from 4 to 5 years in 2024 [F3]"),
    ("life_years_short", 4, "years", "Dropbox's earlier life for infrastructure [F2][F3]"),
    ("life_years_long", 6, "years", "Amazon servers [H13]"),
    ("cost_of_capital", 0.0934, "per year", "WACC, US Software (System & Application), January 2026 [H18]"),
    ("cost_of_capital_low", 0.053, "per year", "weighted-average discount rate on Dropbox's finance leases, which are mostly infrastructure, 2025 [F3]"),
    ("cost_of_capital_high", 0.1066, "per year", "WACC, US Software (Internet), January 2026 [H18]"),
    ("cost_of_capital_none", 0.0, "per year", "cash view, no cost of capital (our inference)"),
]
I = {name: value for name, value, _, _ in INPUTS}
HOURS_PER_YEAR = 24 * 365


def staff_per_year(engineers=None, technicians_on_site=None):
    """FTE and fully loaded cost per year. Constant in 2026 dollars."""
    engineers = I["software_engineers"] if engineers is None else engineers
    on_site = I["technicians_on_site"] if technicians_on_site is None else technicians_on_site
    technicians = I["sites"] * on_site * HOURS_PER_YEAR / I["hours_per_fte"]
    load = 1 / I["wage_share_of_cost"]
    return {
        "engineers_fte": engineers,
        "technicians_fte": technicians,
        "staff_usd": (engineers * I["engineer_salary_usd"]
                      + technicians * I["technician_salary_usd"]) * load,
    }


def capital_recovery(rate, life):
    """Share of the price paid each year to buy an asset over `life` years."""
    if rate == 0:
        return 1 / life
    return rate / (1 - (1 + rate) ** -life)


def cohorts(purchases, life, horizon):
    """Each year's purchases, plus their replacements once `life` runs out.
    Returns (year bought, capex) pairs inside the horizon."""
    out = []
    for year, capex in purchases.items():
        bought = year
        while bought <= horizon:
            out.append((bought, capex))
            bought += life
    return out


def amortize(purchases, horizon, life=None, rate=None):
    """Cash, straight-line depreciation and capital charge per year."""
    life = I["life_years"] if life is None else life
    rate = I["cost_of_capital"] if rate is None else rate
    crf = capital_recovery(rate, life)
    rows = {y: {"capex_cash_usd": 0.0, "depreciation_usd": 0.0, "capital_charge_usd": 0.0}
            for y in range(1, horizon + 1)}
    for bought, capex in cohorts(purchases, life, horizon):
        rows[bought]["capex_cash_usd"] += capex
        for y in range(bought, min(bought + life, horizon + 1)):
            rows[y]["depreciation_usd"] += capex / life
            rows[y]["capital_charge_usd"] += capex * crf
    return rows


def read_hardware(path, price="base"):
    """{start_pb: {year: capex}} from the hardware part's CSV."""
    by_start = {}
    with open(path, newline="") as f:
        for r in csv.DictReader(f):
            capex = float(r[f"server_capex_usd_{price}"]) + float(r["network_capex_usd"])
            by_start.setdefault(float(r["start_pb"]), {})[int(r["year"])] = capex
    return by_start


def main():
    here = Path(__file__).parent
    with open(here / "staff-amortization-inputs.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["input", "value", "unit", "source"])
        w.writerows(INPUTS)

    staff = staff_per_year()
    out = []
    for start_pb, purchases in read_hardware(here / "hardware.csv").items():
        horizon = max(purchases)
        cases = {
            "base": amortize(purchases, horizon),
            "life4": amortize(purchases, horizon, life=I["life_years_short"]),
            "life6": amortize(purchases, horizon, life=I["life_years_long"]),
            "rate_low": amortize(purchases, horizon, rate=I["cost_of_capital_low"]),
            "rate_high": amortize(purchases, horizon, rate=I["cost_of_capital_high"]),
            "cash_rate0": amortize(purchases, horizon, rate=I["cost_of_capital_none"]),
        }
        for year in range(1, horizon + 1):
            row = {
                "start_pb": start_pb,
                "year": year,
                "engineers_fte": staff["engineers_fte"],
                "technicians_fte": round(staff["technicians_fte"], 2),
                "staff_usd": round(staff["staff_usd"]),
            }
            for name, rows in cases.items():
                for key, value in rows[year].items():
                    row[f"{name}_{key}"] = round(value)
            out.append(row)

    with open(here / "staff-amortization.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(out[0]))
        w.writeheader()
        w.writerows(out)

    print(f"staff: {staff['engineers_fte']} engineers, {staff['technicians_fte']:.1f} technicians, "
          f"${staff['staff_usd']:,.0f} a year")
    for start_pb in sorted({r["start_pb"] for r in out}):
        rows = [r for r in out if r["start_pb"] == start_pb]
        cash = sum(r["base_capex_cash_usd"] for r in rows)
        charge = sum(r["base_capital_charge_usd"] for r in rows)
        print(f"{start_pb:>6} PB: capex ${cash:,.0f}, capital charge ${charge:,.0f}, "
              f"staff ${sum(r['staff_usd'] for r in rows):,.0f} over {len(rows)} years")


if __name__ == "__main__":
    main()
