//! `satchel-cost`: prices the case workload (#15) on AWS, Azure, on-prem and
//! Satchel on EC2, and writes the tables the comparison (#26) plots.
//!
//!   cargo run -p satchel-cost                                  # summary table
//!   cargo run -p satchel-cost -- --prices crates/satchel-cost/prices --out docs/cost/output
//!   cargo run -p satchel-cost -- --scenario egress_5x --format json

use std::fmt::Write as _;
use std::path::PathBuf;

use satchel_cost::compare::{self, Case, Inputs, CLOUD_TIERS};
use satchel_cost::layout::PB;
use satchel_cost::prices::{PriceFiles, Prices};

const USAGE: &str =
    "usage: satchel-cost [--prices DIR] [--out DIR] [--scenario NAME|all] [--format table|csv|json]

  --prices    read the price TOML files from DIR (default: the copies built in
              from crates/satchel-cost/prices)
  --out       write every scenario's tables to DIR as CSV: yearly.csv,
              totals.csv, sensitivity.csv, sweep.csv, curve.csv,
              break-even.csv and flagged-inputs.csv. Same as --scenario all
              --format csv
  --scenario  a sensitivity case (default: base). --list shows them all
  --format    table (default) prints a summary, csv writes the tables to
              docs/cost/output, json prints the full comparison";

struct Args {
    scenario: String,
    format: String,
    out: PathBuf,
    prices: Option<PathBuf>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args {
        scenario: "base".to_owned(),
        format: "table".to_owned(),
        out: PathBuf::from("docs/cost/output"),
        prices: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--scenario" => args.scenario = value()?,
            "--format" => args.format = value()?,
            "--out" => {
                args.out = PathBuf::from(value()?);
                args.scenario = "all".to_owned();
                args.format = "csv".to_owned();
            }
            "--prices" => args.prices = Some(PathBuf::from(value()?)),
            "--list" => {
                for case in compare::cases() {
                    println!("{:<20} {}", case.name, case.what);
                }
                return Ok(None);
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(None);
            }
            other => return Err(format!("unknown option {other:?}\n\n{USAGE}")),
        }
    }
    if !["table", "csv", "json"].contains(&args.format.as_str()) {
        return Err(format!("unknown format {:?}\n\n{USAGE}", args.format));
    }
    Ok(Some(args))
}

fn selected(scenario: &str) -> Result<Vec<Case>, String> {
    let cases: Vec<Case> = compare::cases()
        .into_iter()
        .filter(|c| scenario == "all" || c.name == scenario)
        .collect();
    if cases.is_empty() {
        return Err(format!(
            "unknown scenario {scenario:?}: run with --list to see them"
        ));
    }
    Ok(cases)
}

fn load_prices(dir: &std::path::Path) -> Result<Prices, String> {
    let read = |name: &str| {
        let path = dir.join(name);
        std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))
    };
    let (aws, azure, onprem, onprem_25, ec2) = (
        read("aws-s3-ap-south-1.toml")?,
        read("azure-blob-centralindia.toml")?,
        read("onprem.toml")?,
        read("onprem-25.toml")?,
        read("ec2-ap-south-1.toml")?,
    );
    Prices::parse(PriceFiles {
        aws: &aws,
        azure: &azure,
        onprem: &onprem,
        onprem_25: &onprem_25,
        ec2: &ec2,
    })
}

fn inputs_for(base: &Inputs, case: &Case) -> Inputs {
    let mut inputs = base.clone();
    (case.apply)(&mut inputs);
    inputs
}

fn usd(x: f64) -> String {
    format!("{:.1}", x / 1e6)
}

fn pb(bytes: Option<f64>) -> String {
    bytes.map_or("never".to_owned(), |b| format!("{:.2}", b / PB))
}

fn run(args: &Args) -> Result<(), String> {
    let cases = selected(&args.scenario)?;
    let base = match &args.prices {
        Some(dir) => Inputs::with_prices(load_prices(dir)?),
        None => Inputs::base(),
    };
    let mut sensitivity = String::from(
        "scenario,what,aws_total_usd,azure_total_usd,on_prem_total_usd,break_even_vs_aws_pb,break_even_vs_azure_pb\n",
    );
    let mut flagged = String::from("input,value,note\n");
    for f in &base.on_prem.flags {
        writeln!(
            flagged,
            "{},\"{}\",\"{}\"",
            f.input,
            f.value,
            f.note.replace('"', "'")
        )
        .expect("string");
    }
    let mut yearly = String::from("scenario,provider,year,component,usd\n");
    let mut totals = String::from("scenario,provider,name,total_usd\n");
    let mut sweep = String::from("scenario,start_pb,provider,total_usd\n");
    let mut breaks = String::from("scenario,cloud,break_even_start_pb\n");
    let mut curve = String::from("scenario,start_pb,provider,total_usd\n");
    let mut json = Vec::new();

    for case in &cases {
        let inputs = inputs_for(&base, case);
        let c = compare::compare(&inputs)?;
        for (key, cost) in &c.providers {
            for y in &cost.years {
                for (component, v) in y.components.iter().chain(&y.memo) {
                    writeln!(yearly, "{},{key},{},{component},{v:.2}", case.name, y.year)
                        .expect("string");
                }
            }
            writeln!(
                totals,
                "{},{key},\"{}\",{:.2}",
                case.name,
                cost.provider,
                cost.total()
            )
            .expect("string");
        }
        for start in compare::sweep_bytes() {
            let mut i = inputs.clone();
            i.workload = i.workload.with_start(start);
            let s = compare::compare(&i)?;
            for (key, cost) in &s.providers {
                writeln!(
                    sweep,
                    "{},{},{key},{:.2}",
                    case.name,
                    start / PB,
                    cost.total()
                )
                .expect("string");
            }
        }
        for start in compare::curve_bytes() {
            let mut i = inputs.clone();
            i.workload = i.workload.with_start(start);
            let s = compare::compare(&i)?;
            for (key, cost) in &s.providers {
                writeln!(
                    curve,
                    "{},{},{key},{:.2}",
                    case.name,
                    start / PB,
                    cost.total()
                )
                .expect("string");
            }
        }
        let mut row = Vec::new();
        for (key, _, _) in CLOUD_TIERS {
            let at = compare::break_even(&inputs, key)?;
            writeln!(breaks, "{},{key},{}", case.name, pb(at)).expect("string");
            row.push((key, at));
        }
        let at = |key: &str| row.iter().find(|(k, _)| *k == key).and_then(|(_, a)| *a);
        writeln!(
            sensitivity,
            "{},\"{}\",{:.2},{:.2},{:.2},{},{}",
            case.name,
            case.what,
            c.total("aws").unwrap_or(f64::NAN),
            c.total("azure").unwrap_or(f64::NAN),
            c.total("on_prem").unwrap_or(f64::NAN),
            pb(at("aws")),
            pb(at("azure"))
        )
        .expect("string");

        match args.format.as_str() {
            "table" => {
                println!(
                    "{} ({}), {} years, millions of USD",
                    case.name, case.what, inputs.workload.years
                );
                for (key, cost) in &c.providers {
                    println!("  {key:<16} {:>10}   {}", usd(cost.total()), cost.provider);
                }
                for (key, why) in &c.skipped {
                    println!("  {key:<16} {:>10}   {why}", "n/a");
                }
                for (key, at) in row {
                    println!("  break-even vs {key:<16} {:>8} PB at the start", pb(at));
                }
                println!();
            }
            "json" => json.push(serde_json::json!({
                "scenario": case.name,
                "what": case.what,
                "workload": inputs.workload,
                "comparison": c,
            })),
            _ => {}
        }
    }

    if args.format != "json" {
        eprintln!("flagged inputs (proposed, assumed or not yet sourced):");
        for f in &base.on_prem.flags {
            eprintln!("  {}: {} ({})", f.input, f.value, f.note);
        }
    }
    match args.format.as_str() {
        "csv" => {
            std::fs::create_dir_all(&args.out)
                .map_err(|e| format!("creating {}: {e}", args.out.display()))?;
            for (name, body) in [
                ("yearly.csv", &yearly),
                ("totals.csv", &totals),
                ("sweep.csv", &sweep),
                ("break-even.csv", &breaks),
                ("curve.csv", &curve),
                ("sensitivity.csv", &sensitivity),
                ("flagged-inputs.csv", &flagged),
            ] {
                let path = args.out.join(name);
                std::fs::write(&path, body)
                    .map_err(|e| format!("writing {}: {e}", path.display()))?;
                eprintln!("wrote {}", path.display());
            }
        }
        "json" => println!(
            "{}",
            serde_json::to_string_pretty(&json).expect("comparison is JSON")
        ),
        _ => {}
    }
    Ok(())
}

fn main() {
    let result = parse_args().and_then(|args| match args {
        Some(args) => run(&args),
        None => Ok(()),
    });
    if let Err(e) = result {
        eprintln!("satchel-cost: {e}");
        std::process::exit(2);
    }
}
