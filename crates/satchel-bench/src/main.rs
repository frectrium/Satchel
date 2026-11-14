//! `satchel-bench`: runs one experiment configuration and writes its result
//! as JSON. See `docs/experiments/README.md`.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, ValueEnum};
use satchel_bench::{parse_size, run, Backend, Config, Mix};

#[derive(Clone, Copy, ValueEnum)]
enum Target {
    Satchel,
    /// Needs the `s3` feature.
    S3,
}

#[derive(Parser)]
#[command(version, about = "Load generator for Satchel and the S3 baseline")]
struct Args {
    #[arg(long, value_enum, default_value = "satchel")]
    target: Target,
    /// Base URL of satchel-api.
    #[arg(long, default_value = "http://localhost:8080")]
    url: String,
    /// S3 bucket for `--target s3`.
    #[arg(long)]
    s3_bucket: Option<String>,
    #[arg(long, default_value = "ap-south-1")]
    s3_region: String,
    /// Custom S3 endpoint, path-style. Only for tests.
    #[arg(long)]
    s3_endpoint: Option<String>,
    /// Block size, for example 4KiB or 2MiB.
    #[arg(long, default_value = "2MiB", value_parser = parse_size)]
    block_size: u64,
    /// Gets to puts, for example 2:1.
    #[arg(long, default_value = "2:1", value_parser = Mix::parse)]
    mix: Mix,
    #[arg(long, default_value_t = 16)]
    clients: usize,
    /// Seconds of warm-up that isn't measured.
    #[arg(long, default_value_t = 30)]
    warmup: u64,
    /// Measure for at most this many seconds.
    #[arg(long, default_value_t = 300)]
    duration: u64,
    /// Stop measuring after this many operations.
    #[arg(long, default_value_t = 10_000)]
    max_ops: u64,
    /// Data to store before the run, for example 10GiB. Not measured.
    #[arg(long, default_value = "0", value_parser = parse_size)]
    prefill: u64,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// Where to write the JSON result.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let backend = match args.target {
        Target::Satchel => Backend::satchel(&args.url)?,
        Target::S3 => s3(&args)?,
    };
    let config = Config {
        block_size: usize::try_from(args.block_size).context("block size is too large")?,
        mix: args.mix,
        clients: args.clients,
        warmup: Duration::from_secs(args.warmup),
        duration: Duration::from_secs(args.duration),
        max_ops: args.max_ops,
        prefill_bytes: args.prefill,
        seed: args.seed,
    };
    let report = run(backend, config).await?;

    for (op, s) in &report.ops {
        println!(
            "{op:?}: {} ok, {} errors, p50 {:.2} ms, p99 {:.2} ms, p99.9 {:.2} ms, {:.1} ops/s, {:.1} MiB/s",
            s.samples, s.errors, s.p50_ms, s.p99_ms, s.p999_ms, s.ops_per_sec, s.mib_per_sec
        );
    }
    for e in &report.error_examples {
        eprintln!("error: {e}");
    }
    let json = serde_json::to_string_pretty(&report)?;
    match args.out {
        Some(path) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, json + "\n")
                .with_context(|| format!("writing {}", path.display()))?;
            println!("wrote {}", path.display());
        }
        None => println!("{json}"),
    }
    Ok(())
}

#[cfg(feature = "s3")]
fn s3(args: &Args) -> anyhow::Result<Backend> {
    let bucket = args
        .s3_bucket
        .as_deref()
        .context("--target s3 needs --s3-bucket")?;
    Backend::s3(bucket, &args.s3_region, args.s3_endpoint.as_deref())
}

#[cfg(not(feature = "s3"))]
fn s3(_args: &Args) -> anyhow::Result<Backend> {
    anyhow::bail!("this build has no S3 backend: rebuild with `--features s3`")
}
