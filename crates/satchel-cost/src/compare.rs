//! Side-by-side totals, the scale sweep, the break-even data volume and the
//! sensitivity cases that #26 plots (#52).

use serde::Serialize;

use crate::layout::{Ec2Options, PB};
use crate::prices::{self, CloudPrices, Ec2Prices, OnPremPrices};
use crate::pricing::Power;
use crate::pricing::{self, CloudOptions, OnPremOptions, ProviderCost};
use crate::workload::Workload;

/// Every price table and option behind one comparison.
#[derive(Debug, Clone)]
pub struct Inputs {
    pub workload: Workload,
    pub aws: CloudPrices,
    pub azure: CloudPrices,
    pub on_prem: OnPremPrices,
    pub ec2: Ec2Prices,
    pub cloud_regions: u32,
    pub on_prem_options: OnPremOptions,
    pub ec2_options: Ec2Options,
}

impl Inputs {
    /// The workload model's base case with the default price tables.
    pub fn base() -> Self {
        Self::with_prices(prices::Prices::compiled())
    }

    /// The base case with price tables loaded elsewhere (`--prices`).
    pub fn with_prices(p: prices::Prices) -> Self {
        Self {
            workload: Workload::base(),
            on_prem_options: OnPremOptions::base(&p.on_prem),
            ec2_options: ec2_at_scale(&p.ec2),
            aws: p.aws,
            azure: p.azure,
            on_prem: p.on_prem,
            ec2: p.ec2,
            cloud_regions: 1,
        }
    }
}

/// Satchel on EC2 for the case workload: the lab's `m7g.medium` nodes
/// (DDL-015), each with the largest gp3 volume, 64 TB of 1,024 GB [P3], so
/// the node count is set by capacity. Experiment E5 (#77) replaces this with
/// a count set by measured load (our inference until then).
pub fn ec2_at_scale(p: &Ec2Prices) -> Ec2Options {
    Ec2Options {
        ebs_gib_per_node: p.ebs_max_volume_tb * 1024.0,
        ..Ec2Options::lab()
    }
}

/// The options #15 asks to price. The first AWS and Azure entries are the
/// base case; the others use the cheaper cold tier.
pub const CLOUD_TIERS: [(&str, &str, &str); 4] = [
    ("aws", "aws", "standard_ia"),
    ("azure", "azure", "cool"),
    ("aws_glacier_ir", "aws", "glacier_ir"),
    ("azure_cold", "azure", "cold"),
];

#[derive(Debug, Clone, Serialize)]
pub struct Comparison {
    /// Keyed by `aws`, `azure`, `aws_glacier_ir`, `azure_cold`, `on_prem`
    /// and, once priced, `satchel_ec2`.
    pub providers: Vec<(String, ProviderCost)>,
    /// Why a provider has no total, for example missing prices.
    pub skipped: Vec<(String, String)>,
}

impl Comparison {
    pub fn get(&self, key: &str) -> Option<&ProviderCost> {
        self.providers
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, c)| c)
    }

    pub fn total(&self, key: &str) -> Option<f64> {
        self.get(key).map(ProviderCost::total)
    }
}

pub fn compare(inputs: &Inputs) -> Result<Comparison, String> {
    let mut providers = Vec::new();
    for (key, cloud, tier) in CLOUD_TIERS {
        let p = if cloud == "aws" {
            &inputs.aws
        } else {
            &inputs.azure
        };
        let opts = CloudOptions {
            cold_tier: tier.to_owned(),
            regions: inputs.cloud_regions,
        };
        providers.push((key.to_owned(), pricing::cloud(&inputs.workload, p, &opts)?));
    }
    providers.push((
        "on_prem".to_owned(),
        pricing::on_prem(&inputs.workload, &inputs.on_prem, &inputs.on_prem_options)?,
    ));
    let mut skipped = Vec::new();
    match pricing::satchel_ec2(
        &inputs.workload,
        &inputs.ec2,
        &inputs.aws,
        &inputs.ec2_options,
    ) {
        Ok(c) => providers.push(("satchel_ec2".to_owned(), c)),
        Err(why) => skipped.push(("satchel_ec2".to_owned(), why)),
    }
    Ok(Comparison { providers, skipped })
}

/// The starting sizes of #15's sweep.
pub const SWEEP_PB: [f64; 5] = [0.1, 1.0, 10.0, 100.0, 500.0];

/// On-prem total minus a cloud total over the horizon, for one start size.
fn gap(inputs: &Inputs, cloud_key: &str, start_bytes: f64) -> Result<f64, String> {
    let mut i = inputs.clone();
    i.workload = i.workload.with_start(start_bytes);
    let c = compare(&i)?;
    let cloud = c
        .total(cloud_key)
        .ok_or_else(|| format!("no {cloud_key} total"))?;
    Ok(c.total("on_prem").unwrap_or(f64::NAN) - cloud)
}

/// The smallest starting size, between 1 TB and 10 EB, at which owning
/// costs no more than `cloud_key` over the horizon. `None` if owning never
/// wins in that range. Found by bisection on a log scale, to 0.1%.
pub fn break_even(inputs: &Inputs, cloud_key: &str) -> Result<Option<f64>, String> {
    let (mut lo, mut hi) = (1e12_f64, 1e19_f64);
    if gap(inputs, cloud_key, hi)? > 0.0 {
        return Ok(None);
    }
    if gap(inputs, cloud_key, lo)? <= 0.0 {
        return Ok(Some(lo));
    }
    while hi / lo > 1.001 {
        let mid = (lo * hi).sqrt();
        if gap(inputs, cloud_key, mid)? <= 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Ok(Some(hi))
}

/// A named change to the base case.
pub struct Case {
    pub name: &'static str,
    pub what: &'static str,
    pub apply: fn(&mut Inputs),
}

/// The sensitivity cases: ±50% growth and egress (#23, #24), the ranges in
/// #15, section 7, and the on-prem ranges in #25.
pub fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "base",
            what: "workload model base case (#15)",
            apply: |_| {},
        },
        Case {
            name: "growth_minus_50",
            what: "growth 10% a year",
            apply: |i| i.workload.growth *= 0.5,
        },
        Case {
            name: "growth_plus_50",
            what: "growth 30% a year",
            apply: |i| i.workload.growth *= 1.5,
        },
        Case {
            name: "growth_high",
            what: "growth 41% a year (#15, row 3)",
            apply: |i| i.workload.growth = 0.41,
        },
        Case {
            name: "egress_minus_50",
            what: "egress 1x ingest",
            apply: |i| i.workload.egress_ratio *= 0.5,
        },
        Case {
            name: "egress_plus_50",
            what: "egress 3x ingest",
            apply: |i| i.workload.egress_ratio *= 1.5,
        },
        Case {
            name: "egress_5x",
            what: "egress 5x ingest (#15, row 9)",
            apply: |i| i.workload.egress_ratio = 5.0,
        },
        Case {
            name: "block_1mb",
            what: "1 MB average block (#15, row 5)",
            apply: |i| i.workload.avg_block_bytes = 1e6,
        },
        Case {
            name: "block_4mb",
            what: "4 MB average block (#15, row 5)",
            apply: |i| i.workload.avg_block_bytes = 4e6,
        },
        Case {
            name: "two_regions",
            what: "a second cloud region, as Magic Pocket keeps two copies (#15)",
            apply: |i| i.cloud_regions = 2,
        },
        Case {
            name: "server_low",
            what: "server at $32 per raw TB [H3]",
            apply: |i| {
                i.on_prem_options.server_usd_per_raw_tb = i.on_prem.server_usd_per_raw_tb_low.value
            },
        },
        Case {
            name: "server_double",
            what: "server at twice the 2019 price (our inference: drives alone now cost 80% to 126% of it [H2][H3])",
            apply: |i| i.on_prem_options.server_usd_per_raw_tb *= 2.0,
        },
        Case {
            name: "life_4",
            what: "4-year hardware life [F2]",
            apply: |i| i.on_prem_options.life_years = i.on_prem.life_years_low.value as u32,
        },
        Case {
            name: "life_6",
            what: "6-year hardware life [H13]",
            apply: |i| i.on_prem_options.life_years = i.on_prem.life_years_high.value as u32,
        },
        Case {
            name: "capital_low",
            what: "5.3% cost of capital, Dropbox's lease rate [F3]",
            apply: |i| i.on_prem_options.cost_of_capital = i.on_prem.cost_of_capital_low.value,
        },
        Case {
            name: "capital_high",
            what: "10.66% cost of capital, US Software (Internet) WACC [H18]",
            apply: |i| i.on_prem_options.cost_of_capital = i.on_prem.cost_of_capital_high.value,
        },
        Case {
            name: "staff_low",
            what: "8 engineers and 1 technician on site [T9][H17]",
            apply: |i| {
                i.on_prem_options.engineers = i.on_prem.engineers_low.value;
                i.on_prem_options.technicians_on_site = i.on_prem.technicians_on_site_low.value;
            },
        },
        Case {
            name: "power_idle",
            what: "drives at their idle 0.30 W per TB [B12]",
            apply: |i| i.on_prem_options.power = Power::DriveWatts(i.on_prem.drive_w_per_tb_idle.value),
        },
        Case {
            name: "power_rack_budget",
            what: "every rack at its 15 kW budget [B14], an upper bound",
            apply: |i| i.on_prem_options.power = Power::RackBudget,
        },
        Case {
            name: "cold_tier_on_prem",
            what: "Magic Pocket's 2.25x cold tier on-prem [B5][T9]",
            apply: |i| i.on_prem_options.scheme = crate::layout::Scheme::magic_pocket_cold(),
        },
    ]
}

pub fn sweep_bytes() -> impl Iterator<Item = f64> {
    SWEEP_PB.into_iter().map(|pb| pb * PB)
}

/// Starting sizes for the break-even chart (#26): 0.1 PB to 1 EB, eight
/// points a decade.
pub fn curve_bytes() -> impl Iterator<Item = f64> {
    (0..=32).map(|i| 0.1 * PB * 10f64.powf(f64::from(i) / 8.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_case_prices_every_provider() {
        let c = compare(&Inputs::base()).unwrap();
        for key in [
            "aws",
            "azure",
            "aws_glacier_ir",
            "azure_cold",
            "on_prem",
            "satchel_ec2",
        ] {
            assert!(c.total(key).unwrap() > 0.0, "{key}");
        }
        assert!(c.skipped.is_empty());
    }

    #[test]
    fn owning_wins_at_500_pb_and_loses_at_100_tb() {
        let base = Inputs::base();
        assert!(gap(&base, "aws", 500.0 * PB).unwrap() < 0.0);
        assert!(gap(&base, "aws", 0.1 * PB).unwrap() > 0.0);
    }

    #[test]
    fn break_even_lies_between_the_sweep_points_it_separates() {
        let base = Inputs::base();
        let at = break_even(&base, "aws").unwrap().unwrap();
        assert!(gap(&base, "aws", at).unwrap() <= 0.0);
        assert!(gap(&base, "aws", at / 1.01).unwrap() > 0.0);
    }

    #[test]
    fn every_case_runs_and_names_are_unique() {
        let cases = cases();
        let mut names: Vec<&str> = cases.iter().map(|c| c.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), cases.len());
        for case in cases {
            let mut i = Inputs::base();
            (case.apply)(&mut i);
            compare(&i).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        }
    }

    #[test]
    fn egress_moves_the_cloud_bill_and_not_hardware() {
        let base = compare(&Inputs::base()).unwrap();
        let mut i = Inputs::base();
        i.workload.egress_ratio = 5.0;
        let more = compare(&i).unwrap();
        assert!(more.total("aws").unwrap() > base.total("aws").unwrap());
        assert_eq!(
            more.get("on_prem").unwrap().component("hardware"),
            base.get("on_prem").unwrap().component("hardware")
        );
    }
}
