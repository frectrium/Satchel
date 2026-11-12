//! Prices the workload (#15) on AWS, Azure, on-prem and Satchel on EC2 (#52).
//!
//! Cloud bills follow #15, section 5: hot data in the standard tier, cold
//! data in a cold tier that keeps millisecond reads, one lifecycle transition
//! per block a year, and internet egress on the provider's bands. On-prem
//! follows #25: hardware sized by the sizing model (#53) for the data at the
//! end of each year, spread over its life with a cost of capital, plus power,
//! colocation, transit and staff.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::layout::{self, Ec2Options, Hardware, LayoutInput, Scheme, GIB, TB};
use crate::prices::{banded, CloudPrices, Ec2Prices, OnPremPrices};
use crate::workload::{Workload, Year};

const HOURS_PER_YEAR: f64 = 24.0 * 365.0;
const SECONDS_PER_YEAR: f64 = HOURS_PER_YEAR * 3600.0;

/// Cost of one year, by component. `memo` holds figures that are shown but
/// not added to the total, such as cash capex.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YearCost {
    pub year: u32,
    pub components: BTreeMap<&'static str, f64>,
    pub memo: BTreeMap<&'static str, f64>,
}

impl YearCost {
    fn new(year: u32) -> Self {
        Self {
            year,
            components: BTreeMap::new(),
            memo: BTreeMap::new(),
        }
    }

    pub fn total(&self) -> f64 {
        self.components.values().sum()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderCost {
    pub provider: String,
    pub years: Vec<YearCost>,
}

impl ProviderCost {
    pub fn total(&self) -> f64 {
        self.years.iter().map(YearCost::total).sum()
    }

    pub fn component(&self, name: &str) -> f64 {
        self.years
            .iter()
            .filter_map(|y| y.components.get(name))
            .sum()
    }
}

/// Which cold tier, and how many regions hold a full copy (#15: 1, or 2 for
/// the second-copy scenario).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CloudOptions {
    pub cold_tier: String,
    pub regions: u32,
}

/// One year in the cloud.
pub fn cloud_year(y: &Year, p: &CloudPrices, opts: &CloudOptions) -> Result<YearCost, String> {
    let tier = p
        .cold_tiers
        .get(&opts.cold_tier)
        .ok_or_else(|| format!("{} has no cold tier {:?}", p.provider, opts.cold_tier))?;
    let gb = |bytes: f64| bytes / p.bytes_per_gb.value;
    let regions = f64::from(opts.regions);
    let hot_gb = gb(y.avg_bytes * y.hot_fraction);
    let cold_gb = gb(y.avg_bytes * (1.0 - y.hot_fraction));
    let cold_reads = y.gets() * y.cold_read_fraction;

    let mut c = YearCost::new(y.year);
    let mut add = |name, usd| {
        c.components.insert(name, usd);
    };
    add(
        "storage_hot",
        12.0 * banded(hot_gb, 0.0, &p.hot_storage_bands) * regions,
    );
    add(
        "storage_cold",
        12.0 * cold_gb * tier.storage_per_gb_month.value * regions,
    );
    add(
        "requests_put",
        y.puts() / 1000.0 * p.hot_put_per_1000.value * regions,
    );
    add(
        "requests_get",
        (y.gets() - cold_reads) / 1000.0 * p.hot_get_per_1000.value
            + cold_reads / 1000.0 * tier.get_per_1000.value,
    );
    add(
        "transitions",
        y.transitions() / 1000.0 * tier.transition_per_1000.value * regions,
    );
    add(
        "retrieval",
        gb(y.egress_bytes * y.cold_read_fraction) * tier.retrieval_per_gb.value,
    );
    add(
        "egress",
        12.0 * banded(
            gb(y.egress_bytes) / 12.0,
            p.egress_free_gb_per_month.value,
            &p.egress_bands,
        ),
    );
    add(
        "inter_region",
        gb(y.ingest_bytes) * p.inter_region_per_gb.value * (regions - 1.0),
    );
    Ok(c)
}

pub fn cloud(w: &Workload, p: &CloudPrices, opts: &CloudOptions) -> Result<ProviderCost, String> {
    let tier = &p.cold_tiers[&opts.cold_tier].name;
    let regions = if opts.regions > 1 {
        format!(", {} regions", opts.regions)
    } else {
        String::new()
    };
    Ok(ProviderCost {
        provider: format!("{}, cold on {tier}{regions}", p.provider),
        years: w
            .years()
            .iter()
            .map(|y| cloud_year(y, p, opts))
            .collect::<Result<_, _>>()?,
    })
}

/// Choices for the on-prem sensitivity cases (#25, #26).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OnPremOptions {
    pub server_usd_per_raw_tb: f64,
    pub life_years: u32,
    pub cost_of_capital: f64,
    pub engineers: f64,
    pub technicians_on_site: f64,
    pub power: Power,
    pub scheme: Scheme,
    pub hardware: Hardware,
}

/// How the IT load for power and colocation is worked out.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Power {
    /// Drive watts per TB of installed raw capacity [B12] (method.md, section 4).
    DriveWatts(f64),
    /// Every rack at its full power budget [B14]: an upper bound.
    RackBudget,
}

impl OnPremOptions {
    /// The base case of #25: Magic Pocket's layout on 2025 hardware.
    pub fn base(p: &OnPremPrices) -> Self {
        Self {
            server_usd_per_raw_tb: p.server_usd_per_raw_tb.value,
            life_years: p.life_years.value as u32,
            cost_of_capital: p.cost_of_capital.value,
            engineers: p.engineers.value,
            technicians_on_site: p.technicians_on_site.value,
            power: Power::DriveWatts(p.drive_w_per_tb.value),
            scheme: Scheme::magic_pocket(),
            hardware: Hardware::magic_pocket_2025(),
        }
    }
}

/// Share of the price paid each year to buy an asset over `life` years.
pub fn capital_recovery(rate: f64, life: u32) -> f64 {
    if rate == 0.0 {
        1.0 / f64::from(life)
    } else {
        rate / (1.0 - (1.0 + rate).powi(-(life as i32)))
    }
}

/// Staff per year, fully loaded (#25).
pub fn staff_usd(p: &OnPremPrices, engineers: f64, technicians_on_site: f64) -> f64 {
    let technicians = p.sites.value * technicians_on_site * HOURS_PER_YEAR / p.hours_per_fte.value;
    (engineers * p.engineer_salary_usd.value + technicians * p.technician_salary_usd.value)
        / p.wage_share_of_cost.value
}

/// Servers and switches needed for one year's data, across all regions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Fleet {
    pub servers: u64,
    pub racks: u64,
    /// Raw capacity installed.
    pub installed_bytes: f64,
    pub switches: u64,
}

pub fn fleet(y: &Year, p: &OnPremPrices, opts: &OnPremOptions) -> Result<Fleet, String> {
    let input = LayoutInput {
        demand: y.demand_at_end(),
        scheme: opts.scheme,
        hardware: opts.hardware,
        cloud_regions: 1,
        ec2: Ec2Options::lab(),
    };
    let on_prem = layout::layout(&input)?.on_prem;
    let regions = u64::from(on_prem.regions);
    let racks_per_region = on_prem.racks / regions;
    let uplinks = (racks_per_region as f64 * p.rack_uplinks.value / p.switch_ports.value).ceil();
    Ok(Fleet {
        servers: on_prem.servers,
        racks: on_prem.racks,
        installed_bytes: on_prem.raw_bytes,
        switches: (racks_per_region + uplinks as u64) * regions,
    })
}

pub fn on_prem(
    w: &Workload,
    p: &OnPremPrices,
    opts: &OnPremOptions,
) -> Result<ProviderCost, String> {
    if opts.life_years == 0 {
        return Err("life_years must be at least 1".to_owned());
    }
    let years = w.years();
    let fleets: Vec<Fleet> = years
        .iter()
        .map(|y| fleet(y, p, opts))
        .collect::<Result<_, _>>()?;
    let server_usd = opts.hardware.server_bytes() / TB * opts.server_usd_per_raw_tb;

    // Bought at the start of each year for the data at its end (#25).
    let horizon = years.len();
    let mut purchases = vec![0.0; horizon];
    let mut have = Fleet {
        servers: 0,
        racks: 0,
        installed_bytes: 0.0,
        switches: 0,
    };
    for (i, f) in fleets.iter().enumerate() {
        purchases[i] = f.servers.saturating_sub(have.servers) as f64 * server_usd
            + f.switches.saturating_sub(have.switches) as f64 * p.switch_usd.value;
        have = *f;
    }
    // Each purchase is replaced when its life runs out.
    let life = opts.life_years as usize;
    let crf = capital_recovery(opts.cost_of_capital, opts.life_years);
    let mut cash = vec![0.0; horizon];
    let mut charge = vec![0.0; horizon];
    let mut depreciation = vec![0.0; horizon];
    for (first, &capex) in purchases.iter().enumerate() {
        for bought in (first..horizon).step_by(life) {
            cash[bought] += capex;
            for y in bought..(bought + life).min(horizon) {
                charge[y] += capex * crf;
                depreciation[y] += capex / life as f64;
            }
        }
    }

    let staff = staff_usd(p, opts.engineers, opts.technicians_on_site);
    let out = years
        .iter()
        .zip(&fleets)
        .enumerate()
        .map(|(i, (y, f))| {
            let it_kw = match opts.power {
                Power::DriveWatts(w_per_tb) => f.installed_bytes / TB * w_per_tb / 1000.0,
                Power::RackBudget => f.racks as f64 * p.rack_kw.value,
            };
            let egress_mbps = y.egress_bytes * 8.0 / SECONDS_PER_YEAR / 1e6;
            let mut c = YearCost::new(y.year);
            c.components.insert("hardware", charge[i]);
            c.components.insert(
                "power",
                it_kw * HOURS_PER_YEAR * p.pue.value * p.electricity_usd_per_kwh.value,
            );
            c.components.insert(
                "colocation",
                it_kw * p.colocation_usd_per_kw_month.value * 12.0,
            );
            c.components.insert(
                "transit",
                egress_mbps * p.transit_usd_per_mbps_month.value * 12.0,
            );
            c.components.insert("staff", staff);
            c.memo.insert("capex_cash", cash[i]);
            c.memo.insert("depreciation", depreciation[i]);
            c.memo.insert("servers", f.servers as f64);
            c.memo.insert("racks", f.racks as f64);
            c.memo.insert("switches", f.switches as f64);
            c
        })
        .collect();
    Ok(ProviderCost {
        provider: p.provider.clone(),
        years: out,
    })
}

/// Satchel's own design on EC2 (exercise plan, #14), sized by the sizing
/// model. Errors until its prices are in the Source Log.
pub fn satchel_ec2(
    w: &Workload,
    p: &Ec2Prices,
    egress: &CloudPrices,
    ec2: &Ec2Options,
) -> Result<ProviderCost, String> {
    let need = |name: &str, v: &Option<crate::prices::Sourced>| {
        v.as_ref().map(|s| s.value).ok_or_else(|| {
            format!("Satchel on EC2 needs a sourced {name} in prices/ec2-ap-south-1.json")
        })
    };
    let node = need("node_usd_per_hour", &p.node_usd_per_hour)?;
    let control = need("control_usd_per_hour", &p.control_usd_per_hour)?;
    let ebs = need("ebs_usd_per_gb_month", &p.ebs_usd_per_gb_month)?;
    let cross_az = need("cross_az_usd_per_gb", &p.cross_az_usd_per_gb)?;

    let years = w
        .years()
        .iter()
        .map(|y| {
            let input = LayoutInput {
                demand: y.demand_at_end(),
                ec2: ec2.clone(),
                ..LayoutInput::satchel_lab()
            };
            let l = layout::layout(&input)?.ec2;
            let nodes = f64::from(l.nodes);
            let mut c = YearCost::new(y.year);
            c.components
                .insert("instances", (nodes * node + control) * HOURS_PER_YEAR);
            c.components
                .insert("ebs", nodes * l.ebs_gib_per_node * ebs * 12.0);
            c.components
                .insert("cross_az", l.cross_zone_bytes / GIB * cross_az * 2.0);
            c.components.insert(
                "egress",
                12.0 * banded(
                    y.egress_bytes / egress.bytes_per_gb.value / 12.0,
                    egress.egress_free_gb_per_month.value,
                    &egress.egress_bands,
                ),
            );
            c.memo.insert("nodes", nodes);
            Ok(c)
        })
        .collect::<Result<_, String>>()?;
    Ok(ProviderCost {
        provider: p.provider.clone(),
        years,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::PB;
    use crate::prices;

    fn near(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= expected.abs() * tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn s3_standard_sanity_check_from_the_workload_model() {
        // #15 section 6: 500 PB on S3 Standard in Mumbai is about $10.7M a month.
        let p = prices::aws();
        let gb = 500.0 * PB / p.bytes_per_gb.value;
        near(banded(gb, 0.0, &p.hot_storage_bands), 10.71e6, 0.001);
        // Hand check: 51,200 x 0.025 + 460,800 x 0.024 + the rest x 0.023.
        let hand = 51_200.0 * 0.025 + 460_800.0 * 0.024 + (gb - 512_000.0) * 0.023;
        near(banded(gb, 0.0, &p.hot_storage_bands), hand, 1e-12);
    }

    #[test]
    fn egress_bands_and_free_allowance() {
        let p = prices::aws();
        // 200 GB: 100 free, then 100 at $0.1093.
        near(banded(200.0, 100.0, &p.egress_bands), 10.93, 1e-12);
        // 200,000 GB in a month uses every band.
        let gb = 200_000.0;
        let hand = (10_240.0 - 100.0) * 0.1093
            + 40_960.0 * 0.085
            + 102_400.0 * 0.082
            + (gb - 153_600.0) * 0.08;
        near(banded(gb, 100.0, &p.egress_bands), hand, 1e-12);
    }

    #[test]
    fn aws_base_case_year_one_by_hand() {
        let p = prices::aws();
        let y = Workload::base().year(1);
        let opts = CloudOptions {
            cold_tier: "standard_ia".to_owned(),
            regions: 1,
        };
        let c = cloud_year(&y, &p, &opts).unwrap();
        let gb = |b: f64| b / p.bytes_per_gb.value;
        let hot = gb(550.0 * PB / 6.0);
        let cold = gb(550.0 * PB * 5.0 / 6.0);
        near(
            c.components["storage_hot"],
            12.0 * (51_200.0 * 0.025 + 460_800.0 * 0.024 + (hot - 512_000.0) * 0.023),
            1e-9,
        );
        near(c.components["storage_cold"], 12.0 * cold * 0.0138, 1e-9);
        near(c.components["requests_put"], 50e9 / 1000.0 * 0.005, 1e-9);
        near(
            c.components["requests_get"],
            90e9 / 1000.0 * 0.0004 + 10e9 / 1000.0 * 0.001,
            1e-9,
        );
        // Last year's ingest, 83.3 PB, in 2 MB blocks.
        near(
            c.components["transitions"],
            500e15 / 6.0 / 2e6 / 1000.0 * 0.01,
            1e-9,
        );
        near(c.components["retrieval"], gb(20.0 * PB) * 0.01, 1e-9);
        assert_eq!(c.components["inter_region"], 0.0);
    }

    /// The reference figures in docs/cost/method.md, section 6, within $1.
    #[test]
    fn method_reference_figures() {
        let within_a_dollar = |actual: f64, expected: f64| {
            assert!(
                (actual - expected).abs() <= 1.0,
                "expected {expected}, got {actual}"
            )
        };
        let aws = prices::aws();
        let gb = 500.0 * PB / aws.bytes_per_gb.value;
        within_a_dollar(banded(gb, 0.0, &aws.hot_storage_bands), 10_710_773.0);
        let w = Workload::base();
        let opts = |tier: &str| CloudOptions {
            cold_tier: tier.to_owned(),
            regions: 1,
        };
        let aws_cost = cloud(&w, &aws, &opts("standard_ia")).unwrap();
        let azure_cost = cloud(&w, &prices::azure(), &opts("cool")).unwrap();
        within_a_dollar(aws_cost.years[0].total() / 12.0, 9_172_090.0);
        within_a_dollar(azure_cost.years[0].total() / 12.0, 9_180_315.0);
        within_a_dollar(aws_cost.total(), 819_023_325.0);
        within_a_dollar(azure_cost.total(), 819_754_502.0);
    }

    #[test]
    fn aws_year_one_agrees_with_the_calculator_export() {
        // docs/cost/aws/estimate-year-1.json (#23): $110,065,210.92 for 12
        // months. The calculator leaves out the 100 GB of free egress a month.
        let opts = CloudOptions {
            cold_tier: "standard_ia".to_owned(),
            regions: 1,
        };
        let c = cloud_year(&Workload::base().year(1), &prices::aws(), &opts).unwrap();
        near(c.total(), 110_065_210.92, 0.02);
        near(c.total(), 110_065_210.92, 0.0001);
    }

    #[test]
    fn azure_year_one_agrees_with_the_calculator_export() {
        // docs/cost/azure/estimate-year-1.json (#24): $7,963,079.38 a month.
        // The calculator caps data retrieval at 5,000 GB and egress at
        // 500,000 GB a month, so the rest is added at $0.01 and $0.08 a GB.
        let monthly =
            7_963_079.38 + (1_552_204.0 - 5_000.0) * 0.01 + (15_522_043.0 - 500_000.0) * 0.08;
        let opts = CloudOptions {
            cold_tier: "cool".to_owned(),
            regions: 1,
        };
        let c = cloud_year(&Workload::base().year(1), &prices::azure(), &opts).unwrap();
        near(c.total(), 12.0 * monthly, 0.0001);
    }

    #[test]
    fn second_region_doubles_storage_and_pays_for_the_copy() {
        let p = prices::aws();
        let y = Workload::base().year(1);
        let one = cloud_year(
            &y,
            &p,
            &CloudOptions {
                cold_tier: "standard_ia".into(),
                regions: 1,
            },
        )
        .unwrap();
        let two = cloud_year(
            &y,
            &p,
            &CloudOptions {
                cold_tier: "standard_ia".into(),
                regions: 2,
            },
        )
        .unwrap();
        near(
            two.components["storage_cold"],
            2.0 * one.components["storage_cold"],
            1e-12,
        );
        near(
            two.components["inter_region"],
            100.0 * PB / p.bytes_per_gb.value * 0.086,
            1e-12,
        );
        assert_eq!(two.components["egress"], one.components["egress"]);
    }

    #[test]
    fn unknown_cold_tier_is_an_error() {
        let opts = CloudOptions {
            cold_tier: "deep_archive".into(),
            regions: 1,
        };
        assert!(cloud_year(&Workload::base().year(1), &prices::aws(), &opts).is_err());
    }

    #[test]
    fn on_prem_matches_the_hardware_and_staff_parts_of_25() {
        let p = prices::on_prem();
        let c = on_prem(&Workload::base(), &p, &OnPremOptions::base(&p)).unwrap();
        let y1 = &c.years[0];
        // hardware.csv: 564 servers, 72 racks, 108 switches in year 1, and
        // $63,168,000 + $802,718 of capex.
        assert_eq!(y1.memo["servers"], 564.0);
        assert_eq!(y1.memo["racks"], 72.0);
        assert_eq!(y1.memo["switches"], 108.0);
        near(y1.memo["capex_cash"], 63_168_000.0 + 802_718.0, 1e-6);
        let y5 = &c.years[4];
        assert_eq!(y5.memo["servers"], 1168.0);
        assert_eq!(y5.memo["switches"], 220.0);
        // staff-amortization.csv: 5-year capex $132,451,167, capital charge
        // $123,249,050 at 9.34%, staff $4,133,970 a year.
        let cash: f64 = c.years.iter().map(|y| y.memo["capex_cash"]).sum();
        near(cash, 132_451_167.0, 1e-6);
        near(c.component("hardware"), 123_249_050.0, 1e-6);
        near(y1.components["staff"], 4_133_970.0, 1e-6);
    }

    #[test]
    fn short_life_replaces_the_first_purchase() {
        let p = prices::on_prem();
        let mut opts = OnPremOptions::base(&p);
        opts.life_years = 4;
        let c = on_prem(&Workload::base(), &p, &opts).unwrap();
        // staff-amortization.csv, life4: $86,190,291 paid in year 5.
        near(c.years[4].memo["capex_cash"], 86_190_291.0, 1e-6);
    }

    #[test]
    fn capital_recovery_factor() {
        near(capital_recovery(0.053, 5), 0.232_893_573, 1e-8);
        near(capital_recovery(0.0934, 5), 0.259_364_482, 1e-8);
        near(capital_recovery(0.0, 5), 0.2, 1e-12);
    }

    #[test]
    fn power_space_and_transit_by_hand() {
        let p = prices::on_prem();
        let c = on_prem(&Workload::base(), &p, &OnPremOptions::base(&p)).unwrap();
        let y1 = &c.years[0];
        // 564 servers x 3.2 PB installed, at 0.50 W per TB [B12]: 902.4 kW.
        let kw = 564.0 * 3200.0 * 0.5 / 1000.0;
        near(y1.components["power"], kw * 8760.0 * 1.49 * 0.0977, 1e-9);
        near(y1.components["colocation"], kw * 172.5 * 12.0, 1e-9);
        // The rack budget instead: 72 racks x 15 kW.
        let mut opts = OnPremOptions::base(&p);
        opts.power = Power::RackBudget;
        let racks = on_prem(&Workload::base(), &p, &opts).unwrap();
        near(
            racks.years[0].components["power"],
            1080.0 * 8760.0 * 1.49 * 0.0977,
            1e-9,
        );
        let mbps = 200.0 * PB * 8.0 / (8760.0 * 3600.0) / 1e6;
        near(y1.components["transit"], mbps * 0.08 * 12.0, 1e-9);
    }

    #[test]
    fn ec2_needs_sourced_prices() {
        let mut p = prices::ec2();
        p.node_usd_per_hour = None;
        let err =
            satchel_ec2(&Workload::base(), &p, &prices::aws(), &Ec2Options::lab()).unwrap_err();
        assert!(err.contains("prices/ec2-ap-south-1.json"), "{err}");
    }

    #[test]
    fn satchel_lab_cluster_by_hand() {
        // 9 m7g.medium nodes with 50 GiB of gp3 each, plus one m7g.large.
        let w = Workload {
            start_bytes: 10.0 * GIB,
            growth: 0.0,
            years: 1,
            avg_block_bytes: 2.0 * 1_048_576.0,
            egress_ratio: 0.0,
            cold_read_fraction: 0.0,
        };
        let c = satchel_ec2(&w, &prices::ec2(), &prices::aws(), &Ec2Options::lab()).unwrap();
        let y = &c.years[0];
        assert_eq!(y.memo["nodes"], 9.0);
        near(
            y.components["instances"],
            (9.0 * 0.0292 + 0.0583) * 8760.0,
            1e-9,
        );
        near(y.components["ebs"], 9.0 * 50.0 * 0.0912 * 12.0, 1e-9);
        assert_eq!(y.components["cross_az"], 0.0);
    }
}
