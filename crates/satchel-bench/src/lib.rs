//! Load generator for the experiments (#22): the same workload against
//! Satchel or S3, with exact latency percentiles and throughput per
//! operation (exercise plan, sections 4 and 6).

pub mod backend;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use satchel_common::BlockHash;
use serde::Serialize;

pub use backend::Backend;

const KIB: u64 = 1024;

/// Parses a byte size such as `4096`, `4KiB`, `2MiB` or `1GiB`.
pub fn parse_size(raw: &str) -> Result<u64, String> {
    let raw = raw.trim();
    let split = raw.find(|c: char| !c.is_ascii_digit()).unwrap_or(raw.len());
    let (number, unit) = raw.split_at(split);
    let number: u64 = number
        .parse()
        .map_err(|_| format!("{raw:?} is not a size like 4096, 4KiB or 2MiB"))?;
    let scale = match unit.trim() {
        "" | "B" => 1,
        "KiB" => KIB,
        "MiB" => KIB * KIB,
        "GiB" => KIB * KIB * KIB,
        other => return Err(format!("unknown unit {other:?}: use B, KiB, MiB or GiB")),
    };
    number
        .checked_mul(scale)
        .ok_or_else(|| format!("{raw:?} is too large"))
}

/// Gets and puts in each cycle, from `gets:puts` such as `2:1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Mix {
    pub gets: u32,
    pub puts: u32,
}

impl Mix {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let bad = || format!("{raw:?} is not a mix like 2:1 (gets:puts)");
        let (gets, puts) = raw.split_once(':').ok_or_else(bad)?;
        let mix = Self {
            gets: gets.trim().parse().map_err(|_| bad())?,
            puts: puts.trim().parse().map_err(|_| bad())?,
        };
        if mix.gets + mix.puts == 0 {
            return Err(bad());
        }
        Ok(mix)
    }

    /// The `n`th operation of a client: puts first in each cycle, then gets.
    pub fn op(&self, n: u64) -> Op {
        if n % u64::from(self.gets + self.puts) < u64::from(self.puts) {
            Op::Put
        } else {
            Op::Get
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    Put,
    Get,
}

#[derive(Debug, Clone, Serialize)]
pub struct Config {
    pub block_size: usize,
    pub mix: Mix,
    pub clients: usize,
    /// Not measured.
    pub warmup: Duration,
    /// Measuring stops after this long, or after `max_ops`.
    pub duration: Duration,
    pub max_ops: u64,
    /// Data stored before the run starts, not measured (E2).
    pub prefill_bytes: u64,
    pub seed: u64,
}

/// Seeded xorshift64*, so a run can be repeated exactly.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // splitmix64 spreads nearby seeds apart.
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Self((z ^ (z >> 31)) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

/// Block number `n` of `client`: unique and the same on every run with the
/// same seed.
pub fn block(seed: u64, client: u64, n: u64, size: usize) -> Vec<u8> {
    let mut rng = Rng::new(seed ^ client.rotate_left(21) ^ n.rotate_left(42));
    let mut out = Vec::with_capacity(size + 8);
    for word in [seed, client, n] {
        out.extend_from_slice(&word.to_le_bytes());
    }
    while out.len() < size {
        out.extend_from_slice(&rng.next().to_le_bytes());
    }
    out.truncate(size);
    out
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpStats {
    /// Successful operations measured.
    pub samples: usize,
    pub errors: usize,
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p99_ms: f64,
    pub p999_ms: f64,
    pub mean_ms: f64,
    pub max_ms: f64,
    pub ops_per_sec: f64,
    pub mib_per_sec: f64,
}

/// Nearest-rank percentile of sorted latencies.
pub fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let rank = (p * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn stats(mut latencies: Vec<Duration>, errors: usize, bytes: u64, window: Duration) -> OpStats {
    latencies.sort();
    let secs = window.as_secs_f64().max(f64::EPSILON);
    let total: Duration = latencies.iter().sum();
    OpStats {
        samples: latencies.len(),
        errors,
        p50_ms: ms(percentile(&latencies, 0.50)),
        p90_ms: ms(percentile(&latencies, 0.90)),
        p99_ms: ms(percentile(&latencies, 0.99)),
        p999_ms: ms(percentile(&latencies, 0.999)),
        mean_ms: if latencies.is_empty() {
            0.0
        } else {
            ms(total) / latencies.len() as f64
        },
        max_ms: ms(latencies.last().copied().unwrap_or_default()),
        ops_per_sec: latencies.len() as f64 / secs,
        mib_per_sec: bytes as f64 / (KIB * KIB) as f64 / secs,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub target: String,
    pub config: Config,
    /// Length of the measured window.
    pub measured_secs: f64,
    /// Up to the first 20 distinct error messages.
    pub error_examples: Vec<String>,
    pub ops: BTreeMap<Op, OpStats>,
}

struct Sample {
    op: Op,
    latency: Duration,
    result: Result<u64, String>,
}

/// Runs the workload against `backend`: prefill, warm-up, then measurement.
pub async fn run(backend: Backend, config: Config) -> anyhow::Result<Report> {
    anyhow::ensure!(config.clients > 0, "clients must be at least 1");
    anyhow::ensure!(config.block_size > 0, "block size must be at least 1 byte");
    let backend = Arc::new(backend);
    let stored = Arc::new(RwLock::new(Vec::<BlockHash>::new()));

    // Prefill, plus one block per client if the mix starts with no puts.
    let prefill_blocks = config.prefill_bytes.div_ceil(config.block_size as u64);
    let prefill_blocks = if config.mix.gets > 0 {
        prefill_blocks.max(config.clients as u64)
    } else {
        prefill_blocks
    };
    let mut tasks = Vec::new();
    for client in 0..config.clients as u64 {
        let (backend, stored, config) = (backend.clone(), stored.clone(), config.clone());
        tasks.push(tokio::spawn(async move {
            let mut n = client;
            while n < prefill_blocks {
                let data = block(config.seed, u64::MAX - client, n, config.block_size);
                let hash = BlockHash::of(&data);
                backend
                    .put(&hash, data)
                    .await
                    .map_err(|e| anyhow::anyhow!("prefill put failed: {e}"))?;
                stored.write().expect("lock").push(hash);
                n += config.clients as u64;
            }
            anyhow::Ok(())
        }));
    }
    for task in tasks {
        task.await??;
    }

    let start = Instant::now();
    let measure_from = start + config.warmup;
    let stop_at = measure_from + config.duration;
    let measured = Arc::new(AtomicU64::new(0));
    let mut tasks = Vec::new();
    for client in 0..config.clients as u64 {
        let (backend, stored, config, measured) = (
            backend.clone(),
            stored.clone(),
            config.clone(),
            measured.clone(),
        );
        tasks.push(tokio::spawn(async move {
            let mut rng = Rng::new(config.seed ^ client);
            let mut samples = Vec::new();
            let mut last_end = measure_from;
            for n in 0.. {
                let now = Instant::now();
                if now >= stop_at || measured.load(Ordering::Relaxed) >= config.max_ops {
                    break;
                }
                let op = config.mix.op(n);
                // Built before the clock starts, so data generation isn't timed.
                let (hash, data) = match op {
                    Op::Put => {
                        let data = block(config.seed, client, n, config.block_size);
                        (BlockHash::of(&data), Some(data))
                    }
                    Op::Get => {
                        let stored = stored.read().expect("lock");
                        (stored[(rng.next() % stored.len() as u64) as usize], None)
                    }
                };
                let began = Instant::now();
                let result = match data {
                    Some(data) => {
                        let len = data.len() as u64;
                        backend.put(&hash, data).await.map(|()| len)
                    }
                    None => backend.get(&hash).await.map(|len| len as u64),
                };
                let ended = Instant::now();
                if op == Op::Put && result.is_ok() {
                    stored.write().expect("lock").push(hash);
                }
                if began >= measure_from {
                    measured.fetch_add(1, Ordering::Relaxed);
                    last_end = ended;
                    samples.push(Sample {
                        op,
                        latency: ended - began,
                        result,
                    });
                }
            }
            (samples, last_end)
        }));
    }

    let mut samples = Vec::new();
    let mut window_end = measure_from;
    for task in tasks {
        let (s, end) = task.await?;
        samples.extend(s);
        window_end = window_end.max(end);
    }
    let window = window_end.saturating_duration_since(measure_from);

    let mut error_examples = Vec::new();
    let mut ops = BTreeMap::new();
    for op in [Op::Put, Op::Get] {
        let (mut latencies, mut errors, mut bytes) = (Vec::new(), 0, 0);
        for s in samples.iter().filter(|s| s.op == op) {
            match &s.result {
                Ok(n) => {
                    latencies.push(s.latency);
                    bytes += n;
                }
                Err(e) => {
                    errors += 1;
                    if error_examples.len() < 20 && !error_examples.contains(e) {
                        error_examples.push(e.clone());
                    }
                }
            }
        }
        if !latencies.is_empty() || errors > 0 {
            ops.insert(op, stats(latencies, errors, bytes, window));
        }
    }
    Ok(Report {
        target: backend.name().to_owned(),
        config,
        measured_secs: window.as_secs_f64(),
        error_examples,
        ops,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sizes() {
        assert_eq!(parse_size("4096"), Ok(4096));
        assert_eq!(parse_size("4KiB"), Ok(4096));
        assert_eq!(parse_size("2MiB"), Ok(2 << 20));
        assert_eq!(parse_size("1GiB"), Ok(1 << 30));
        assert_eq!(parse_size(" 64 B "), Ok(64));
        for bad in ["", "MiB", "2MB", "-1", "2.5MiB"] {
            assert!(parse_size(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_mix_and_spreads_ops() {
        let mix = Mix::parse("2:1").unwrap();
        let ops: Vec<Op> = (0..6).map(|n| mix.op(n)).collect();
        assert_eq!(ops, [Op::Put, Op::Get, Op::Get, Op::Put, Op::Get, Op::Get]);
        assert!((0..10).all(|n| Mix::parse("0:1").unwrap().op(n) == Op::Put));
        assert!((0..10).all(|n| Mix::parse("1:0").unwrap().op(n) == Op::Get));
        for bad in ["0:0", "2", "a:1", "1:-1"] {
            assert!(Mix::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn blocks_are_repeatable_and_unique() {
        assert_eq!(block(1, 2, 3, 4096), block(1, 2, 3, 4096));
        assert_ne!(block(1, 2, 3, 4096), block(1, 2, 4, 4096));
        assert_ne!(block(1, 2, 3, 4096), block(1, 3, 3, 4096));
        assert_ne!(block(1, 2, 3, 4096), block(2, 2, 3, 4096));
        assert_eq!(block(1, 2, 3, 1).len(), 1);
        assert_eq!(block(1, 2, 3, 4_194_304).len(), 4_194_304);
    }

    #[test]
    fn nearest_rank_percentiles() {
        let sorted: Vec<Duration> = (1..=1000).map(Duration::from_millis).collect();
        assert_eq!(percentile(&sorted, 0.50), Duration::from_millis(500));
        assert_eq!(percentile(&sorted, 0.99), Duration::from_millis(990));
        assert_eq!(percentile(&sorted, 0.999), Duration::from_millis(999));
        assert_eq!(percentile(&sorted[..1], 0.999), Duration::from_millis(1));
        assert_eq!(percentile(&[], 0.5), Duration::ZERO);
    }

    #[test]
    fn stats_from_latencies() {
        let latencies = vec![Duration::from_millis(10); 100];
        let s = stats(latencies, 3, 100 << 20, Duration::from_secs(10));
        assert_eq!(s.samples, 100);
        assert_eq!(s.errors, 3);
        assert!((s.ops_per_sec - 10.0).abs() < 1e-9);
        assert!((s.mib_per_sec - 10.0).abs() < 1e-9);
        assert!((s.p99_ms - 10.0).abs() < 1e-9);
    }
}
