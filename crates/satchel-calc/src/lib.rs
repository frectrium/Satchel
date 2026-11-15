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
    fn defaults_carry_price_sources() {
        let d = defaults_json();
        assert!(d.contains("\"source\""));
    }
}
