//! Price tables (#52). The defaults are the TOML files in `prices/`,
//! compiled in so the library still does no I/O; the CLI can load another
//! copy with `--prices`. Every value carries its Source Log ID, and every
//! input that is proposed, assumed or not yet sourced is returned as a
//! [`Flag`] instead of being used silently.

use std::collections::BTreeMap;

use serde::Serialize;
use toml::Value;

/// A number and where it comes from.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sourced {
    pub value: f64,
    pub source: String,
}

/// One volume band: the price applies up to `up_to_gb` a month, counted
/// from zero. `None` is the open-ended last band.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Band {
    pub up_to_gb: Option<f64>,
    pub usd: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColdTier {
    pub name: String,
    pub storage_per_gb_month: Sourced,
    pub get_per_1000: Sourced,
    pub retrieval_per_gb: Sourced,
    /// Moving one object into this tier.
    pub transition_per_1000: Sourced,
}

/// Object storage list prices for one cloud region.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CloudPrices {
    pub provider: String,
    pub bytes_per_gb: Sourced,
    pub hot_storage_bands: Vec<Band>,
    pub hot_put_per_1000: Sourced,
    pub hot_get_per_1000: Sourced,
    pub cold_tiers: BTreeMap<String, ColdTier>,
    pub egress_free_gb_per_month: Sourced,
    pub egress_bands: Vec<Band>,
    pub inter_region_per_gb: Sourced,
}

/// An input the engine uses that isn't settled: proposed, assumed, a
/// placeholder or waiting on another issue.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Flag {
    pub input: String,
    pub value: String,
    pub note: String,
}

/// Owning the storage: hardware (#25, Parshv), power, space and transit
/// (#25, Guru) and staff and amortization (#25, Jas).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OnPremPrices {
    pub provider: String,
    pub server_usd_per_raw_tb: Sourced,
    pub server_usd_per_raw_tb_low: Sourced,
    pub switch_usd: Sourced,
    pub switch_ports: Sourced,
    pub rack_uplinks: Sourced,
    pub rack_kw: Sourced,
    pub drive_w_per_tb: Sourced,
    pub drive_w_per_tb_idle: Sourced,
    pub pue: Sourced,
    pub electricity_usd_per_kwh: Sourced,
    pub colocation_usd_per_kw_month: Sourced,
    pub transit_usd_per_mbps_month: Sourced,
    pub engineers: Sourced,
    pub engineers_low: Sourced,
    pub engineer_salary_usd: Sourced,
    pub sites: Sourced,
    pub technicians_on_site: Sourced,
    pub technicians_on_site_low: Sourced,
    pub hours_per_fte: Sourced,
    pub technician_salary_usd: Sourced,
    pub wage_share_of_cost: Sourced,
    pub life_years: Sourced,
    pub life_years_low: Sourced,
    pub life_years_high: Sourced,
    pub cost_of_capital: Sourced,
    pub cost_of_capital_low: Sourced,
    pub cost_of_capital_high: Sourced,
    pub flags: Vec<Flag>,
}

/// Satchel's own cluster on EC2.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ec2Prices {
    pub provider: String,
    pub node_usd_per_hour: Option<Sourced>,
    pub control_usd_per_hour: Option<Sourced>,
    pub ebs_usd_per_gb_month: Option<Sourced>,
    /// Charged once in each direction.
    pub cross_az_usd_per_gb: Option<Sourced>,
    pub ebs_max_volume_tb: f64,
}

/// The TOML sources for every price table.
#[derive(Debug, Clone, Copy)]
pub struct PriceFiles<'a> {
    pub aws: &'a str,
    pub azure: &'a str,
    pub onprem: &'a str,
    pub onprem_25: &'a str,
    pub ec2: &'a str,
}

impl PriceFiles<'static> {
    /// The files in `prices/`, compiled in.
    pub fn compiled() -> Self {
        Self {
            aws: include_str!("../prices/aws-s3-ap-south-1.toml"),
            azure: include_str!("../prices/azure-blob-centralindia.toml"),
            onprem: include_str!("../prices/onprem.toml"),
            onprem_25: include_str!("../prices/onprem-25.toml"),
            ec2: include_str!("../prices/ec2-ap-south-1.toml"),
        }
    }
}

/// All price tables, parsed.
#[derive(Debug, Clone)]
pub struct Prices {
    pub aws: CloudPrices,
    pub azure: CloudPrices,
    pub on_prem: OnPremPrices,
    pub ec2: Ec2Prices,
}

impl Prices {
    pub fn parse(files: PriceFiles<'_>) -> Result<Self, String> {
        let doc = |name: &str, text: &str| -> Result<Doc, String> {
            toml::from_str::<toml::Table>(text)
                .map(|t| Doc {
                    name: name.to_owned(),
                    root: Value::Table(t),
                })
                .map_err(|e| format!("prices/{name} is not valid TOML: {e}"))
        };
        let aws = doc("aws-s3-ap-south-1.toml", files.aws)?;
        let azure = doc("azure-blob-centralindia.toml", files.azure)?;
        let onprem = doc("onprem.toml", files.onprem)?;
        let onprem_25 = doc("onprem-25.toml", files.onprem_25)?;
        let ec2 = doc("ec2-ap-south-1.toml", files.ec2)?;
        Ok(Self {
            aws: parse_aws(&aws)?,
            azure: parse_azure(&azure)?,
            on_prem: parse_on_prem(&onprem, &onprem_25)?,
            ec2: parse_ec2(&ec2)?,
        })
    }

    /// The compiled-in defaults.
    pub fn compiled() -> Self {
        Self::parse(PriceFiles::compiled()).expect("the price files in prices/ parse")
    }
}

pub fn aws() -> CloudPrices {
    Prices::compiled().aws
}

pub fn azure() -> CloudPrices {
    Prices::compiled().azure
}

pub fn on_prem() -> OnPremPrices {
    Prices::compiled().on_prem
}

pub fn ec2() -> Ec2Prices {
    Prices::compiled().ec2
}

struct Doc {
    name: String,
    root: Value,
}

impl Doc {
    fn value(&self, path: &str) -> Result<&Value, String> {
        let mut v = &self.root;
        for key in path.split('.') {
            v = v
                .get(key)
                .ok_or_else(|| format!("prices/{} has no {path}", self.name))?;
        }
        Ok(v)
    }

    fn num(&self, path: &str) -> Result<f64, String> {
        match self.value(path)? {
            Value::Float(f) => Ok(*f),
            Value::Integer(i) => Ok(*i as f64),
            _ => Err(format!("prices/{}: {path} is not a number", self.name)),
        }
    }

    fn text(&self, path: &str) -> Result<String, String> {
        self.value(path)?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("prices/{}: {path} is not text", self.name))
    }

    /// A number tagged with the `source` of its table, or of `source_key`.
    fn sourced(&self, path: &str, source_key: &str) -> Result<Sourced, String> {
        Ok(Sourced {
            value: self.num(path)?,
            source: self.text(source_key)?,
        })
    }

    fn bands(&self, path: &str, price_key: &str) -> Result<Vec<Band>, String> {
        let list = self
            .value(path)?
            .as_array()
            .ok_or_else(|| format!("prices/{}: {path} is not a list", self.name))?;
        list.iter()
            .map(|b| {
                let num = |k: &str| match b.get(k) {
                    Some(Value::Float(f)) => Some(*f),
                    Some(Value::Integer(i)) => Some(*i as f64),
                    _ => None,
                };
                Ok(Band {
                    up_to_gb: num("up_to_gb"),
                    usd: num(price_key).ok_or_else(|| {
                        format!("prices/{}: a band in {path} has no {price_key}", self.name)
                    })?,
                })
            })
            .collect()
    }
}

fn parse_aws(d: &Doc) -> Result<CloudPrices, String> {
    let tier = |name: &str, key: &str, retrieval: &str| -> Result<ColdTier, String> {
        Ok(ColdTier {
            name: name.to_owned(),
            storage_per_gb_month: d.sourced(
                &format!("storage.{key}.usd_per_gb_month"),
                &format!("storage.{key}.source"),
            )?,
            get_per_1000: d.sourced(&format!("requests.{key}.get"), "requests.source")?,
            retrieval_per_gb: d.sourced(
                &format!("retrieval.{retrieval}.usd_per_gb"),
                "retrieval.source",
            )?,
            transition_per_1000: d
                .sourced(&format!("requests.{key}.lifecycle_in"), "requests.source")?,
        })
    };
    Ok(CloudPrices {
        provider: "AWS S3, Asia Pacific (Mumbai)".to_owned(),
        bytes_per_gb: d.sourced("meta.gb_bytes", "meta.source")?,
        hot_storage_bands: d.bands("storage.standard.bands", "usd_per_gb_month")?,
        hot_put_per_1000: d.sourced("requests.standard.put", "requests.source")?,
        hot_get_per_1000: d.sourced("requests.standard.get", "requests.source")?,
        cold_tiers: [
            (
                "standard_ia".to_owned(),
                tier("S3 Standard-IA", "standard_ia", "standard_ia")?,
            ),
            (
                "glacier_ir".to_owned(),
                tier(
                    "S3 Glacier Instant Retrieval",
                    "glacier_instant_retrieval",
                    "glacier_instant_retrieval",
                )?,
            ),
        ]
        .into(),
        egress_free_gb_per_month: d.sourced(
            "transfer.internet_out.free_gb_per_month",
            "transfer.internet_out.source",
        )?,
        egress_bands: d.bands("transfer.internet_out.bands", "usd_per_gb")?,
        inter_region_per_gb: d.sourced(
            "transfer.other.to_other_aws_region_usd_per_gb",
            "transfer.other.source",
        )?,
    })
}

fn parse_azure(d: &Doc) -> Result<CloudPrices, String> {
    // Azure prices operations per 10,000; the engine works per 1,000.
    let per_1000 = |path: &str| -> Result<Sourced, String> {
        let s = d.sourced(path, "operations.source")?;
        Ok(Sourced {
            value: s.value / 10.0,
            ..s
        })
    };
    let tier = |name: &str, key: &str| -> Result<ColdTier, String> {
        Ok(ColdTier {
            name: name.to_owned(),
            storage_per_gb_month: d.sourced(
                &format!("storage.{key}.zrs"),
                &format!("storage.{key}.source"),
            )?,
            get_per_1000: per_1000(&format!("operations.{key}_zrs.read"))?,
            retrieval_per_gb: d.sourced(&format!("retrieval.{key}"), "retrieval.source")?,
            // A tier change is billed as a write to the new tier (our
            // inference, method.md section 3).
            transition_per_1000: per_1000(&format!("operations.{key}_zrs.write"))?,
        })
    };
    // Egress bands are listed by their lower edge. The first, at $0, is the
    // free allowance.
    let edges = d.bands("transfer.internet_out.microsoft_network", "usd_per_gb")?;
    let from: Vec<f64> = d
        .value("transfer.internet_out.microsoft_network")?
        .as_array()
        .into_iter()
        .flatten()
        .map(|b| b.get("from_gb").and_then(Value::as_integer).unwrap_or(0) as f64)
        .collect();
    let free = if edges.first().map(|b| b.usd) == Some(0.0) {
        from.get(1).copied().unwrap_or(0.0)
    } else {
        0.0
    };
    let egress_bands = (0..edges.len())
        .filter(|&i| edges[i].usd > 0.0)
        .map(|i| Band {
            up_to_gb: from.get(i + 1).copied(),
            usd: edges[i].usd,
        })
        .collect();
    Ok(CloudPrices {
        provider: "Azure Blob Storage, Central India, ZRS".to_owned(),
        // Blob Storage is billed in binary GB, 2^30 bytes [P4].
        bytes_per_gb: Sourced {
            value: 1_073_741_824.0,
            source: "P4".to_owned(),
        },
        hot_storage_bands: d.bands("storage.hot.zrs", "usd_per_gb_month")?,
        hot_put_per_1000: per_1000("operations.hot_zrs.write")?,
        hot_get_per_1000: per_1000("operations.hot_zrs.read")?,
        cold_tiers: [
            ("cool".to_owned(), tier("Azure Cool ZRS", "cool")?),
            ("cold".to_owned(), tier("Azure Cold ZRS", "cold")?),
        ]
        .into(),
        egress_free_gb_per_month: Sourced {
            value: free,
            source: d.text("transfer.internet_out.source")?,
        },
        egress_bands,
        inter_region_per_gb: d.sourced(
            "transfer.other.inter_region_usd_per_gb",
            "transfer.other.source",
        )?,
    })
}

fn parse_on_prem(o: &Doc, p: &Doc) -> Result<OnPremPrices, String> {
    let fixed = |value: f64, source: &str| Sourced {
        value,
        source: source.to_owned(),
    };
    let per_pb = o.sourced(
        "hardware.server_with_drives_usd_per_pb.value",
        "hardware.server_with_drives_usd_per_pb.source",
    )?;
    let mut flags = vec![
        Flag {
            input: "capital.wacc".into(),
            value: format!("{}", o.num("capital.wacc.base")?),
            note: "proposed in onprem.toml [H18]".into(),
        },
        Flag {
            input: "staff.employer_overhead_multiplier".into(),
            value: format!("1 / {} (onprem.toml has 1.0)", p.num("staff.wage_share_of_cost")?),
            note: "onprem.toml's placeholder is replaced by the sourced wage share [H15]".into(),
        },
        Flag {
            input: "space.colo.nova_usd_per_kw_month".into(),
            value: format!("{}", p.num("defaults_pending_guru.colo_usd_per_kw_month")?),
            note: "midpoint of [H7]'s range, per month assumed (our calc). Power, space and network are Guru's part of #25".into(),
        },
        Flag {
            input: "network.transit".into(),
            value: "average egress, no peak factor".into(),
            note: "a peak-to-average factor has no source (our inference)".into(),
        },
        Flag {
            input: "hardware.drive_power_w_per_tb".into(),
            value: "active".into(),
            note: "the higher of [B12]'s two figures; drives only, no server overhead (our inference)".into(),
        },
    ];
    if let Some(Value::Table(pending)) = o.root.get("pending") {
        for (key, note) in pending {
            let value = if key == "satchel_on_ec2" {
                "priced from AWS Pricing Calculator readings [P3] in ec2-ap-south-1.toml"
            } else {
                "not used"
            };
            flags.push(Flag {
                input: format!("pending.{key}"),
                value: value.into(),
                note: note.as_str().unwrap_or_default().to_owned(),
            });
        }
    }
    Ok(OnPremPrices {
        provider: "On-prem, Magic Pocket layout, US".to_owned(),
        server_usd_per_raw_tb: Sourced {
            value: per_pb.value / 1000.0,
            source: per_pb.source,
        },
        server_usd_per_raw_tb_low: p.sourced(
            "hardware.server_usd_per_raw_tb_low",
            "hardware.source_server_low",
        )?,
        switch_usd: p.sourced("hardware.switch_usd", "hardware.source_switch")?,
        switch_ports: p.sourced("hardware.switch_ports", "hardware.source_switch")?,
        rack_uplinks: fixed(p.num("hardware.rack_uplinks")?, "our inference"),
        rack_kw: o.sourced("hardware.rack.kw_per_rack", "hardware.rack.source")?,
        drive_w_per_tb: o.sourced(
            "hardware.drive_power_w_per_tb.active",
            "hardware.drive_power_w_per_tb.source",
        )?,
        drive_w_per_tb_idle: o.sourced(
            "hardware.drive_power_w_per_tb.idle",
            "hardware.drive_power_w_per_tb.source",
        )?,
        pue: o.sourced("power.pue.north_america", "power.pue.source")?,
        electricity_usd_per_kwh: o.sourced(
            "power.usd_per_kwh.us_industrial",
            "power.usd_per_kwh.source",
        )?,
        colocation_usd_per_kw_month: fixed(
            p.num("defaults_pending_guru.colo_usd_per_kw_month")?,
            "H7",
        ),
        transit_usd_per_mbps_month: o.sourced(
            "network.transit_400g_median_usd_per_mbps_month",
            "network.source_transit",
        )?,
        engineers: fixed(p.num("staff.engineers_base")?, "T9"),
        engineers_low: o.sourced("staff.storage_engineers.low", "staff.source_headcount")?,
        engineer_salary_usd: p
            .sourced("staff.engineer_salary_usd", "staff.source_engineer_salary")?,
        sites: fixed(p.num("staff.sites")?, "B2, B5"),
        technicians_on_site: p
            .sourced("staff.technicians_on_site.base", "staff.source_technicians")?,
        technicians_on_site_low: p
            .sourced("staff.technicians_on_site.low", "staff.source_technicians")?,
        hours_per_fte: fixed(p.num("staff.hours_per_fte")?, "our calc"),
        technician_salary_usd: o
            .sourced("staff.us_sysadmin_median_usd_per_year", "staff.source_us")?,
        wage_share_of_cost: p.sourced("staff.wage_share_of_cost", "staff.source_wage_share")?,
        life_years: o.sourced(
            "amortization.server_years.base",
            "amortization.source_hyperscaler",
        )?,
        life_years_low: o.sourced(
            "amortization.server_years.low",
            "amortization.source_dropbox",
        )?,
        life_years_high: o.sourced(
            "amortization.server_years.high",
            "amortization.source_hyperscaler",
        )?,
        cost_of_capital: o.sourced("capital.wacc.base", "capital.source")?,
        cost_of_capital_low: p
            .sourced("capital.dropbox_lease_rate", "capital.source_lease_rate")?,
        cost_of_capital_high: o.sourced("capital.wacc.high", "capital.source")?,
        flags,
    })
}

fn parse_ec2(d: &Doc) -> Result<Ec2Prices, String> {
    let s = |path: &str| d.sourced(path, "meta.source").ok();
    Ok(Ec2Prices {
        provider: "Satchel on EC2, Asia Pacific (Mumbai)".to_owned(),
        node_usd_per_hour: s("instances.node.usd_per_hour"),
        control_usd_per_hour: s("instances.control.usd_per_hour"),
        ebs_usd_per_gb_month: s("ebs.gp3.usd_per_gb_month"),
        cross_az_usd_per_gb: s("transfer.inter_az_usd_per_gb_each_way"),
        ebs_max_volume_tb: d.num("ebs.gp3.max_volume_tb")?,
    })
}

/// Cost of `gb` in one month across volume bands, after `free_gb`.
pub fn banded(gb: f64, free_gb: f64, bands: &[Band]) -> f64 {
    let mut total = 0.0;
    let mut from = free_gb.min(gb);
    for band in bands {
        let to = band.up_to_gb.map_or(gb, |up| up.min(gb));
        if to > from {
            total += (to - from) * band.usd;
            from = to;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_parses() {
        let p = Prices::compiled();
        assert_eq!(p.aws.hot_storage_bands.len(), 3);
        assert!(p.aws.cold_tiers.contains_key("standard_ia"));
        assert!(p.azure.cold_tiers.contains_key("cool"));
        assert_eq!(p.on_prem.life_years.value, 5.0);
        assert_eq!(p.on_prem.cost_of_capital.value, 0.0934);
        assert!(p.ec2.node_usd_per_hour.is_some());
    }

    #[test]
    fn azure_units_and_egress_edges() {
        let a = azure();
        // $0.06875 per 10,000 writes is $0.006875 per 1,000.
        assert!((a.hot_put_per_1000.value - 0.006875).abs() < 1e-12);
        assert_eq!(a.egress_free_gb_per_month.value, 100.0);
        assert_eq!(a.egress_bands[0].up_to_gb, Some(10335.0));
        assert_eq!(a.egress_bands.last().unwrap().up_to_gb, None);
        assert_eq!(a.egress_bands.last().unwrap().usd, 0.08);
    }

    #[test]
    fn unsettled_inputs_are_flagged() {
        let flags = on_prem().flags;
        for input in [
            "capital.wacc",
            "pending.spares_fraction",
            "pending.server_cost_2026",
            "staff.employer_overhead_multiplier",
        ] {
            assert!(
                flags.iter().any(|f| f.input == input),
                "{input} not flagged"
            );
        }
    }

    #[test]
    fn bad_files_say_which_value_is_missing() {
        let mut files = PriceFiles::compiled();
        files.ec2 = "[meta]\nsource = \"P3\"\n";
        let err = Prices::parse(files).unwrap_err();
        assert!(err.contains("ec2-ap-south-1.toml"), "{err}");
    }

    #[test]
    fn bands_are_counted_from_zero() {
        let bands = [
            Band {
                up_to_gb: Some(10.0),
                usd: 1.0,
            },
            Band {
                up_to_gb: None,
                usd: 0.5,
            },
        ];
        assert_eq!(banded(4.0, 0.0, &bands), 4.0);
        assert_eq!(banded(30.0, 0.0, &bands), 10.0 + 10.0);
        // The free allowance comes off the first band.
        assert_eq!(banded(30.0, 2.0, &bands), 8.0 + 10.0);
        assert_eq!(banded(1.0, 2.0, &bands), 0.0);
    }
}
