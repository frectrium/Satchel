//! Pricing calculator (#54): the `satchel-cost` engine behind a JSON API.
//!
//! The page in `web/calculator/` loads this crate compiled to WASM and calls
//! [`defaults_json`] once and [`calculate_json`] on every change. Native
//! tests call the same functions, so the page and the CLI never disagree.

use serde::{Deserialize, Serialize};

use satchel_cost::compare::{self, Inputs};
use satchel_cost::layout::{self, Layout, LayoutInput, Scheme};
use satchel_cost::pricing::ProviderCost;
use satchel_cost::workload::Workload;

/// Everything the page can change. Missing fields take the base case (#15).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Request {
    pub workload: Workload,
    pub scheme: SchemeChoice,
    pub on_prem: OnPremInputs,
    /// Cloud regions holding a full copy: 1, or 2 for the second-copy case.
    pub cloud_regions: u32,
    /// Also work out the sensitivity table, which takes longer.
    pub sensitivity: bool,
}

/// The on-prem layout. Cloud prices don't depend on it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "preset", rename_all = "snake_case")]
pub enum SchemeChoice {
    /// Two regions, RS(6,3) each, 3.0x raw [B2][T8][T9].
    #[default]
    MagicPocket,
    /// The later 2 + 1 cold tier, 2.25x raw [B5][T9].
    MagicPocketCold,
    /// One region over 3 zones, 3 replicas while open, RS(6,3) once closed.
    Satchel,
    /// Magic Pocket's layout with other replica and code parameters.
    Custom {
        open_replicas: u32,
        k: u32,
        m: u32,
        regions: u32,
    },
}

impl SchemeChoice {
    pub fn scheme(&self) -> Scheme {
        match *self {
            Self::MagicPocket => Scheme::magic_pocket(),
            Self::MagicPocketCold => Scheme::magic_pocket_cold(),
            Self::Satchel => Scheme::satchel(),
            Self::Custom {
                open_replicas,
                k,
                m,
                regions,
            } => Scheme {
                open_replicas,
                k,
                m,
                region_data: 1,
                region_parity: regions.saturating_sub(1),
                ..Scheme::magic_pocket()
            },
        }
    }
}

/// The on-prem choices the page exposes (#25).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OnPremInputs {
    pub server_usd_per_raw_tb: f64,
    pub life_years: u32,
    pub cost_of_capital: f64,
    pub engineers: f64,
    pub technicians_on_site: f64,
}

impl Default for Request {
    fn default() -> Self {
        let base = Inputs::base();
        Self {
            workload: base.workload,
            scheme: SchemeChoice::MagicPocket,
            on_prem: OnPremInputs {
                server_usd_per_raw_tb: base.on_prem_options.server_usd_per_raw_tb,
                life_years: base.on_prem_options.life_years,
                cost_of_capital: base.on_prem_options.cost_of_capital,
                engineers: base.on_prem_options.engineers,
                technicians_on_site: base.on_prem_options.technicians_on_site,
            },
            cloud_regions: base.cloud_regions,
            sensitivity: false,
        }
    }
}

impl Default for OnPremInputs {
    fn default() -> Self {
        Request::default().on_prem
    }
}

impl Request {
    /// The engine's inputs for this request.
    pub fn inputs(&self) -> Inputs {
        let mut i = Inputs::base();
        i.workload = self.workload;
        i.cloud_regions = self.cloud_regions;
        let o = &mut i.on_prem_options;
        o.scheme = self.scheme.scheme();
        o.server_usd_per_raw_tb = self.on_prem.server_usd_per_raw_tb;
        o.life_years = self.on_prem.life_years;
        o.cost_of_capital = self.on_prem.cost_of_capital;
        o.engineers = self.on_prem.engineers;
        o.technicians_on_site = self.on_prem.technicians_on_site;
        i
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Provider {
    pub key: String,
    pub name: String,
    pub total_usd: f64,
    /// Cost of each year, in order.
    pub yearly_usd: Vec<f64>,
    pub cost: ProviderCost,
}

#[derive(Debug, Clone, Serialize)]
pub struct BreakEven {
    pub cloud: String,
    /// Smallest starting data size at which owning costs no more, or none
    /// if owning never wins between 1 TB and 10 EB.
    pub start_bytes: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SensitivityRow {
    pub name: String,
    pub what: String,
    pub break_even_vs_aws_bytes: Option<f64>,
    pub break_even_vs_azure_bytes: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub providers: Vec<Provider>,
    pub skipped: Vec<(String, String)>,
    pub break_even: Vec<BreakEven>,
    /// The layout at the end of the horizon, when the data is largest.
    pub layout: Layout,
    pub sensitivity: Vec<SensitivityRow>,
}

/// Display names for the comparison's provider keys.
pub fn provider_name(key: &str) -> &str {
    match key {
        "aws" => "AWS S3, Standard-IA for cold data",
        "azure" => "Azure Blob, Cool for cold data",
        "aws_glacier_ir" => "AWS S3, Glacier Instant Retrieval for cold data",
        "azure_cold" => "Azure Blob, Cold for cold data",
        "on_prem" => "On-prem, owned hardware",
        "satchel_ec2" => "Satchel on EC2",
        other => other,
    }
}

pub fn calculate(request: &Request) -> Result<Response, String> {
    let inputs = request.inputs();
    let comparison = compare::compare(&inputs)?;
    let providers = comparison
        .providers
        .iter()
        .map(|(key, cost)| Provider {
            key: key.clone(),
            name: provider_name(key).to_owned(),
            total_usd: cost.total(),
            yearly_usd: cost.years.iter().map(|y| y.total()).collect(),
            cost: cost.clone(),
        })
        .collect();
    let break_even = ["aws", "azure"]
        .into_iter()
        .map(|cloud| {
            Ok(BreakEven {
                cloud: cloud.to_owned(),
                start_bytes: compare::break_even(&inputs, cloud)?,
            })
        })
        .collect::<Result<_, String>>()?;
    let last = inputs.workload.year(inputs.workload.years.max(1));
    let layout = layout::layout(&LayoutInput {
        demand: last.demand_at_end(),
        scheme: inputs.on_prem_options.scheme,
        hardware: inputs.on_prem_options.hardware,
        cloud_regions: inputs.cloud_regions,
        ec2: inputs.ec2_options.clone(),
    })?;
    let sensitivity = if request.sensitivity {
        compare::cases()
            .into_iter()
            .map(|case| {
                let mut i = inputs.clone();
                (case.apply)(&mut i);
                Ok(SensitivityRow {
                    name: case.name.to_owned(),
                    what: case.what.to_owned(),
                    break_even_vs_aws_bytes: compare::break_even(&i, "aws")?,
                    break_even_vs_azure_bytes: compare::break_even(&i, "azure")?,
                })
            })
            .collect::<Result<_, String>>()?
    } else {
        Vec::new()
    };
    Ok(Response {
        providers,
        skipped: comparison.skipped,
        break_even,
        layout,
        sensitivity,
    })
}

/// What the page loads first: the base-case request and the price tables
/// with their Source Log IDs.
#[derive(Debug, Clone, Serialize)]
pub struct Defaults {
    pub request: Request,
    pub prices: serde_json::Value,
}

pub fn defaults() -> Defaults {
    let i = Inputs::base();
    Defaults {
        request: Request::default(),
        prices: serde_json::json!({
            "aws": i.aws,
            "azure": i.azure,
            "on_prem": i.on_prem,
            "ec2": i.ec2,
        }),
    }
}

pub fn defaults_json() -> String {
    serde_json::to_string(&defaults()).expect("defaults serialize")
}

/// Parses a [`Request`], runs [`calculate`] and returns the [`Response`].
pub fn calculate_json(request: &str) -> Result<String, String> {
    let request: Request =
        serde_json::from_str(request).map_err(|e| format!("bad request: {e}"))?;
    let response = calculate(&request)?;
    serde_json::to_string(&response).map_err(|e| e.to_string())
}

/// What a handful of uploaded files cost per month, for the website's live
/// panel: S3 Standard at its first price band, against owning the same
/// bytes in Satchel's layout. The on-prem side is the marginal cost of the
/// capacity only: hardware paid off over its life, drive power and
/// colocation. Staff, switches and transit are fixed costs that only show
/// at scale, so the calculator's full model covers them *(our inference)*.
#[derive(Debug, Clone, Serialize)]
pub struct FilesCost {
    pub logical_bytes: f64,
    pub s3: S3FilesCost,
    pub satchel: SatchelFilesCost,
}

#[derive(Debug, Clone, Serialize)]
pub struct S3FilesCost {
    /// AWS bills binary GB (2^30 bytes) [P1].
    pub billed_gb: f64,
    pub usd_per_gb_month: f64,
    pub storage_usd_month: f64,
    /// One PUT per object, paid once.
    pub puts_usd: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SatchelFilesCost {
    /// Raw bytes once volumes are closed and coded: RS(6,3) is 1.5x.
    pub raw_bytes: f64,
    pub overhead: f64,
    /// Raw bytes while volumes are still open: 3 replicas.
    pub open_raw_bytes: f64,
    pub hardware_usd_month: f64,
    pub power_usd_month: f64,
    pub colocation_usd_month: f64,
    pub total_usd_month: f64,
    pub sources: Vec<(String, String)>,
}

const HOURS_PER_MONTH: f64 = 730.0;

pub fn files_cost(logical_bytes: f64, objects: f64) -> Result<FilesCost, String> {
    if !(logical_bytes >= 0.0 && objects >= 0.0) {
        return Err("sizes must be zero or more".into());
    }
    let i = Inputs::base();
    let aws = &i.aws;
    let first = aws
        .hot_storage_bands
        .first()
        .ok_or("the AWS price table has no storage bands")?;
    let billed_gb = logical_bytes / aws.bytes_per_gb.value;
    let s3 = S3FilesCost {
        billed_gb,
        usd_per_gb_month: first.usd,
        storage_usd_month: billed_gb * first.usd,
        puts_usd: objects / 1000.0 * aws.hot_put_per_1000.value,
        source: aws.hot_put_per_1000.source.clone(),
    };

    let scheme = Scheme::satchel();
    let raw = logical_bytes * scheme.coded_overhead();
    let tb = raw / 1e12;
    let p = &i.on_prem;
    let o = &i.on_prem_options;
    let recovery = satchel_cost::pricing::capital_recovery(o.cost_of_capital, o.life_years);
    let kw = tb * p.drive_w_per_tb.value / 1000.0;
    let hardware = tb * o.server_usd_per_raw_tb * recovery / 12.0;
    let power = kw * HOURS_PER_MONTH * p.pue.value * p.electricity_usd_per_kwh.value;
    let colocation = kw * p.colocation_usd_per_kw_month.value;
    let satchel = SatchelFilesCost {
        raw_bytes: raw,
        overhead: scheme.coded_overhead(),
        open_raw_bytes: logical_bytes * f64::from(scheme.open_replicas),
        hardware_usd_month: hardware,
        power_usd_month: power,
        colocation_usd_month: colocation,
        total_usd_month: hardware + power + colocation,
        sources: [
            ("server price per raw TB", &p.server_usd_per_raw_tb.source),
            ("drive watts per TB", &p.drive_w_per_tb.source),
            ("PUE", &p.pue.source),
            ("electricity per kWh", &p.electricity_usd_per_kwh.source),
            (
                "colocation per kW-month",
                &p.colocation_usd_per_kw_month.source,
            ),
            ("hardware life", &p.life_years.source),
            ("cost of capital", &p.cost_of_capital.source),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.clone()))
        .collect(),
    };
    Ok(FilesCost {
        logical_bytes,
        s3,
        satchel,
    })
}

/// [`files_cost`] from and to JSON: `{"logical_bytes": .., "objects": ..}`.
pub fn files_cost_json(request: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Req {
        logical_bytes: f64,
        #[serde(default)]
        objects: f64,
    }
    let r: Req = serde_json::from_str(request).map_err(|e| format!("bad request: {e}"))?;
    serde_json::to_string(&files_cost(r.logical_bytes, r.objects)?).map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn defaults() -> String {
        super::defaults_json()
    }

    #[wasm_bindgen]
    pub fn calculate(request: &str) -> Result<String, JsError> {
        super::calculate_json(request).map_err(|e| JsError::new(&e))
    }

    #[wasm_bindgen]
    pub fn files_cost(request: &str) -> Result<String, JsError> {
        super::files_cost_json(request).map_err(|e| JsError::new(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_request_is_the_cli_base_case() {
        assert_eq!(
            Request::default().inputs().workload,
            Inputs::base().workload
        );
        let ours = calculate(&Request::default()).unwrap();
        let cli = compare::compare(&Inputs::base()).unwrap();
        assert_eq!(ours.providers.len(), cli.providers.len());
        for (p, (key, cost)) in ours.providers.iter().zip(&cli.providers) {
            assert_eq!(&p.key, key);
            assert_eq!(p.total_usd, cost.total(), "{key}");
        }
        assert_eq!(
            ours.break_even[0].start_bytes,
            compare::break_even(&Inputs::base(), "aws").unwrap()
        );
    }

    #[test]
    fn an_empty_request_is_the_base_case() {
        let empty = calculate_json("{}").unwrap();
        let base = calculate_json(&serde_json::to_string(&Request::default()).unwrap()).unwrap();
        assert_eq!(empty, base);
    }

    #[test]
    fn requests_round_trip_through_json() {
        let r = Request {
            scheme: SchemeChoice::Custom {
                open_replicas: 3,
                k: 10,
                m: 4,
                regions: 1,
            },
            cloud_regions: 2,
            ..Request::default()
        };
        let back: Request = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn more_data_costs_more_everywhere() {
        let small = calculate(&Request::default()).unwrap();
        let mut big = Request::default();
        big.workload.start_bytes *= 2.0;
        let big = calculate(&big).unwrap();
        for (s, b) in small.providers.iter().zip(&big.providers) {
            assert!(b.total_usd > s.total_usd, "{}", s.key);
        }
        assert!(big.layout.on_prem.servers > small.layout.on_prem.servers);
    }

    #[test]
    fn the_satchel_scheme_changes_on_prem_only() {
        let mp = calculate(&Request::default()).unwrap();
        let satchel = calculate(&Request {
            scheme: SchemeChoice::Satchel,
            ..Request::default()
        })
        .unwrap();
        let total = |r: &Response, key: &str| {
            r.providers
                .iter()
                .find(|p| p.key == key)
                .map(|p| p.total_usd)
                .unwrap()
        };
        assert_eq!(total(&mp, "aws"), total(&satchel, "aws"));
        assert_ne!(total(&mp, "on_prem"), total(&satchel, "on_prem"));
    }

    #[test]
    fn sensitivity_covers_every_case() {
        let r = calculate(&Request {
            sensitivity: true,
            ..Request::default()
        })
        .unwrap();
        assert_eq!(r.sensitivity.len(), compare::cases().len());
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        assert!(calculate_json("not json").is_err());
        let mut r = Request::default();
        r.workload.avg_block_bytes = 0.0;
        assert!(calculate_json(&serde_json::to_string(&r).unwrap()).is_err());
    }

    #[test]
    fn files_cost_grows_with_the_data_and_cites_sources() {
        let gib = 1_073_741_824.0;
        let one = files_cost(gib, 1.0).unwrap();
        assert!((one.s3.billed_gb - 1.0).abs() < 1e-12);
        assert!((one.s3.storage_usd_month - one.s3.usd_per_gb_month).abs() < 1e-12);
        assert_eq!(one.satchel.overhead, 1.5);
        assert_eq!(one.satchel.raw_bytes, 1.5 * gib);
        assert_eq!(one.satchel.open_raw_bytes, 3.0 * gib);
        assert!(one.satchel.total_usd_month > 0.0);
        let two = files_cost(2.0 * gib, 2.0).unwrap();
        assert!((two.s3.storage_usd_month - 2.0 * one.s3.storage_usd_month).abs() < 1e-12);
        assert!((two.satchel.total_usd_month - 2.0 * one.satchel.total_usd_month).abs() < 1e-9);
        assert!(one.satchel.sources.iter().all(|(_, s)| !s.is_empty()));
        assert!(files_cost(-1.0, 0.0).is_err());
        assert!(files_cost_json(r#"{"logical_bytes": 1000}"#).is_ok());
    }

    #[test]
    fn defaults_carry_price_sources() {
        let d = defaults_json();
        assert!(d.contains("\"source\""));
    }
}
