//! Sizing model (#53): turns one year of a workload into the blocks, volumes,
//! fragments, drives, servers, racks, cloud tiers and EC2 nodes it needs.
//!
//! Units are decimal bytes (1 PB = 10^15) unless a name says MiB or GiB.
//! Defaults cite Source Log IDs. Values marked "our calc" or "our inference"
//! are worked out by us, not disclosed by Dropbox.

use serde::{Deserialize, Serialize};

pub const GB: f64 = 1e9;
pub const TB: f64 = 1e12;
pub const PB: f64 = 1e15;
pub const MIB: f64 = 1_048_576.0;
pub const GIB: f64 = 1_073_741_824.0;

const SECONDS_PER_YEAR: f64 = 365.0 * 24.0 * 3600.0;

/// One year of the workload, before any redundancy. The cost engine builds
/// one per year of the horizon from the workload model (#15).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Demand {
    /// Average data stored over the year.
    pub stored_bytes: f64,
    /// Average block size.
    pub avg_block_bytes: f64,
    /// New data written during the year.
    pub ingest_bytes: f64,
    /// Data served to users during the year.
    pub egress_bytes: f64,
    /// Share of stored data written in the last 12 months.
    pub hot_fraction: f64,
    /// Share of reads that hit data older than 12 months.
    pub cold_read_fraction: f64,
}

impl Demand {
    /// Year 1 of the workload model's base case (#15) for a starting size:
    /// 20% growth [T5], 2 MB blocks and egress of twice the ingest (both
    /// assumptions in #15), and 10% of reads on cold data [B5].
    pub fn base_case_year_one(start_bytes: f64) -> Self {
        let growth = 0.20;
        let ingest = start_bytes * growth;
        Self {
            stored_bytes: start_bytes + ingest / 2.0,
            avg_block_bytes: 2e6,
            ingest_bytes: ingest,
            egress_bytes: 2.0 * ingest,
            // Data written in the last year is g / (1 + g) of the total (our calc).
            hot_fraction: growth / (1.0 + growth),
            cold_read_fraction: 0.10,
        }
    }
}

/// How data is laid out for durability.
///
/// Copies across regions are written as a code too: 1 data + 1 parity is two
/// full copies, and 2 + 1 is Dropbox's cold tier, two halves plus an XOR
/// parity in three regions [B5]. Inside each region, closed volumes use
/// RS(k, m) and open volumes are replicated.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Scheme {
    pub region_data: u32,
    pub region_parity: u32,
    /// Failure domains (zones or AZs) per region that fragments spread over.
    pub zones_per_region: u32,
    /// Replicas per region of an open volume, before it is coded.
    pub open_replicas: u32,
    /// Data fragments per closed volume.
    pub k: u32,
    /// Parity fragments per closed volume.
    pub m: u32,
    pub bucket_bytes: f64,
    /// Data held by one closed volume, before coding.
    pub volume_bytes: f64,
    /// How long new data stays in open, replicated volumes. 0 counts all
    /// data as coded.
    pub open_window_secs: f64,
}

impl Scheme {
    /// Magic Pocket as the workload model prices it (#15): two regions, each
    /// RS(6,3), so 3.0x raw [B2][T8][T9]. Open volumes have 4 replicas per
    /// zone [T8][T9]. Buckets are 1 GB [B2]. A volume is 6 extents of 1 GB
    /// [B4] (our inference). Dropbox doesn't say how long volumes stay open,
    /// so all data is counted as coded.
    pub fn magic_pocket() -> Self {
        Self {
            region_data: 1,
            region_parity: 1,
            zones_per_region: 1,
            open_replicas: 4,
            k: 6,
            m: 3,
            bucket_bytes: GB,
            volume_bytes: 6.0 * GB,
            open_window_secs: 0.0,
        }
    }

    /// Magic Pocket's cold tier: 2 + 1 across three regions [B5], each RS(6,3)
    /// [T9], so 2.25x raw (our calc). A later optimisation, not the 2016 layout.
    pub fn magic_pocket_cold() -> Self {
        Self {
            region_data: 2,
            region_parity: 1,
            ..Self::magic_pocket()
        }
    }

    /// Satchel (exercise plan, #14): one region over 3 AZs, 3 replicas while
    /// open (DDL-006), RS(6,3) once closed (DDL-007), 64 MiB volumes of one
    /// bucket, closed after 10 minutes (DDL-004).
    pub fn satchel() -> Self {
        Self {
            region_data: 1,
            region_parity: 0,
            zones_per_region: 3,
            open_replicas: 3,
            k: 6,
            m: 3,
            bucket_bytes: 64.0 * MIB,
            volume_bytes: 64.0 * MIB,
            open_window_secs: 600.0,
        }
    }

    pub fn regions(&self) -> u32 {
        self.region_data + self.region_parity
    }

    fn region_factor(&self) -> f64 {
        f64::from(self.regions()) / f64::from(self.region_data)
    }

    /// Raw bytes per byte of coded data.
    pub fn coded_overhead(&self) -> f64 {
        self.region_factor() * f64::from(self.k + self.m) / f64::from(self.k)
    }

    /// Raw bytes per byte of data in open volumes.
    pub fn open_overhead(&self) -> f64 {
        self.region_factor() * f64::from(self.open_replicas)
    }

    /// Fragments of one closed volume, over every region.
    pub fn fragments_per_volume(&self) -> u32 {
        (self.k + self.m) * self.regions()
    }

    /// Fragments of a volume that one region holds.
    fn fragments_per_region(&self) -> u32 {
        self.k + self.m
    }
}

/// Storage server hardware for the on-prem layout.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Hardware {
    pub drive_bytes: f64,
    pub drives_per_server: u32,
    pub servers_per_rack: u32,
    pub rack_kw: f64,
    pub server_nic_gbps: f64,
    pub cell_raw_bytes: f64,
    /// Extra raw capacity bought on top of what the data needs.
    pub spare_fraction: f64,
}

impl Hardware {
    /// The 2018 SMR chassis: 100 drives of 14 TB in 4U, 8 per rack, 50 Gbps
    /// [B4][T7]. 2016 cells of about 50 PB raw [B2]. The rack power budget is
    /// the 15 kW from 2025 [B14], since no 2018 figure is published.
    pub fn magic_pocket_2018() -> Self {
        Self {
            drive_bytes: 14.0 * TB,
            drives_per_server: 100,
            servers_per_rack: 8,
            rack_kw: 15.0,
            server_nic_gbps: 50.0,
            cell_raw_bytes: 50.0 * PB,
            spare_fraction: 0.0,
        }
    }

    /// Seventh-generation hardware: drives up to 32 TB, about 200 Gbps per
    /// system, 15 kW per rack [B14]. "More than 100 drives" and 8 chassis per
    /// rack from the 6th generation [B9], so 100 is a lower bound. Cells of
    /// 100 to 120 PB [T9], so 100 PB.
    pub fn magic_pocket_2025() -> Self {
        Self {
            drive_bytes: 32.0 * TB,
            drives_per_server: 100,
            servers_per_rack: 8,
            rack_kw: 15.0,
            server_nic_gbps: 200.0,
            cell_raw_bytes: 100.0 * PB,
            spare_fraction: 0.0,
        }
    }

    pub fn server_bytes(&self) -> f64 {
        self.drive_bytes * f64::from(self.drives_per_server)
    }
}

/// The EC2 cluster Satchel runs on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ec2Options {
    pub instance_type: String,
    pub ebs_gib_per_node: f64,
    pub min_nodes_per_zone: u32,
}

impl Ec2Options {
    /// The measured setup: `m7g.medium` nodes (DDL-015) with a 50 GiB gp3
    /// volume each, 3 per AZ (exercise plan, #14).
    pub fn lab() -> Self {
        Self {
            instance_type: "m7g.medium".to_owned(),
            ebs_gib_per_node: 50.0,
            min_nodes_per_zone: 3,
        }
    }
}

/// Everything the sizing model needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutInput {
    pub demand: Demand,
    /// Layout for on-prem. Satchel on EC2 always uses [`Scheme::satchel`].
    pub scheme: Scheme,
    pub hardware: Hardware,
    /// Cloud regions holding a full copy: 1, or 2 for the second-copy scenario (#15).
    pub cloud_regions: u32,
    pub ec2: Ec2Options,
}

impl LayoutInput {
    /// The workload model's base case (#15): Dropbox at 500 PB, priced on
    /// current hardware since #15 uses 2026 prices.
    pub fn dropbox_base_case() -> Self {
        Self {
            demand: Demand::base_case_year_one(500.0 * PB),
            scheme: Scheme::magic_pocket(),
            hardware: Hardware::magic_pocket_2025(),
            cloud_regions: 1,
            ec2: Ec2Options::lab(),
        }
    }

    /// The Satchel lab cluster: the largest dataset in experiment E2, 50 GiB
    /// (exercise plan, #14), written once and read back twice.
    pub fn satchel_lab() -> Self {
        let stored = 50.0 * GIB;
        Self {
            demand: Demand {
                stored_bytes: stored,
                avg_block_bytes: 2.0 * MIB,
                ingest_bytes: stored,
                egress_bytes: 2.0 * stored,
                hot_fraction: 1.0,
                cold_read_fraction: 0.0,
            },
            scheme: Scheme::satchel(),
            hardware: Hardware::magic_pocket_2025(),
            cloud_regions: 1,
            ec2: Ec2Options::lab(),
        }
    }
}

/// Logical units, the same wherever the data lives.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Logical {
    pub blocks: u64,
    pub buckets: u64,
    pub volumes: u64,
    pub fragments_per_volume: u32,
    pub fragments: u64,
    pub fragment_bytes: f64,
    /// Data still in open, replicated volumes.
    pub open_bytes: f64,
    pub raw_bytes: f64,
    /// Raw bytes per stored byte.
    pub overhead: f64,
}

/// Hardware for owning the storage, Magic Pocket style.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OnPrem {
    pub regions: u32,
    /// Raw capacity installed, spares included.
    pub raw_bytes: f64,
    pub drives: u64,
    pub servers: u64,
    pub racks: u64,
    pub cells: u64,
    /// Racks times the rack budget: an upper bound on power draw.
    pub power_budget_kw: f64,
    /// Total server bandwidth.
    pub nic_gbps: f64,
    /// Average egress to users.
    pub serving_gbps: f64,
    /// Dropbox's standard of 30 Gbps per PB [B14], applied to raw capacity
    /// (our inference: 200 Gbps per system [B14] over 6 PB enclosures [B12]
    /// is about 33 Gbps per raw PB).
    pub standard_gbps: f64,
    /// Time to rebuild one failed server from the rest of its cell, if every
    /// other server gives it its full bandwidth. RS reads k fragments per
    /// lost one (our calc).
    pub repair_hours_one_server: Option<f64>,
}

/// Cloud object storage, per region. Tier names are the pricing engine's job.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Cloud {
    pub regions: u32,
    pub hot_bytes: f64,
    pub cold_bytes: f64,
    pub put_requests: f64,
    pub hot_get_requests: f64,
    pub cold_get_requests: f64,
    /// Blocks moved from the hot to the cold tier in the year: last year's
    /// ingest (#15).
    pub transitions: f64,
    pub cold_retrieval_bytes: f64,
    pub egress_bytes: f64,
    /// Ingest copied to the other regions.
    pub inter_region_bytes: f64,
}

/// Satchel's own cluster on EC2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ec2 {
    pub instance_type: String,
    pub zones: u32,
    pub nodes_per_zone: u32,
    pub nodes: u32,
    pub ebs_gib_per_node: f64,
    pub raw_bytes: f64,
    /// Bytes crossing AZs in the year: 2 of 3 replicas on put, the fragments
    /// an encode writes to other zones, and reads served from another zone
    /// (our calc).
    pub cross_zone_bytes: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub logical: Logical,
    pub on_prem: OnPrem,
    pub cloud: Cloud,
    pub ec2: Ec2,
}

/// Sizes the workload. Errors name the input that is out of range.
pub fn layout(input: &LayoutInput) -> Result<Layout, String> {
    validate(input)?;
    let logical = size_logical(&input.demand, &input.scheme);
    let satchel = Scheme::satchel();
    Ok(Layout {
        on_prem: on_prem(&input.demand, &input.scheme, &input.hardware, &logical),
        cloud: cloud(&input.demand, input.cloud_regions),
        ec2: ec2(
            &input.demand,
            &satchel,
            &size_logical(&input.demand, &satchel),
            &input.ec2,
        ),
        logical,
    })
}

fn validate(input: &LayoutInput) -> Result<(), String> {
    let d = &input.demand;
    let s = &input.scheme;
    let h = &input.hardware;
    let non_negative = [
        ("stored_bytes", d.stored_bytes),
        ("ingest_bytes", d.ingest_bytes),
        ("egress_bytes", d.egress_bytes),
        ("open_window_secs", s.open_window_secs),
        ("spare_fraction", h.spare_fraction),
    ];
    let positive = [
        ("avg_block_bytes", d.avg_block_bytes),
        ("bucket_bytes", s.bucket_bytes),
        ("volume_bytes", s.volume_bytes),
        ("drive_bytes", h.drive_bytes),
        ("rack_kw", h.rack_kw),
        ("server_nic_gbps", h.server_nic_gbps),
        ("cell_raw_bytes", h.cell_raw_bytes),
        ("ebs_gib_per_node", input.ec2.ebs_gib_per_node),
    ];
    let fractions = [
        ("hot_fraction", d.hot_fraction),
        ("cold_read_fraction", d.cold_read_fraction),
    ];
    let counts = [
        ("region_data", s.region_data),
        ("zones_per_region", s.zones_per_region),
        ("open_replicas", s.open_replicas),
        ("k", s.k),
        ("drives_per_server", h.drives_per_server),
        ("servers_per_rack", h.servers_per_rack),
        ("cloud_regions", input.cloud_regions),
    ];
    for (name, v) in non_negative {
        if !(v.is_finite() && v >= 0.0) {
            return Err(format!("{name} must be 0 or more, got {v}"));
        }
    }
    for (name, v) in positive {
        if !(v.is_finite() && v > 0.0) {
            return Err(format!("{name} must be above 0, got {v}"));
        }
    }
    for (name, v) in fractions {
        if !(0.0..=1.0).contains(&v) {
            return Err(format!("{name} must be between 0 and 1, got {v}"));
        }
    }
    for (name, v) in counts {
        if v == 0 {
            return Err(format!("{name} must be at least 1"));
        }
    }
    Ok(())
}

fn ceil(x: f64) -> u64 {
    x.ceil() as u64
}

fn size_logical(demand: &Demand, scheme: &Scheme) -> Logical {
    let stored = demand.stored_bytes;
    let open = (demand.ingest_bytes / SECONDS_PER_YEAR * scheme.open_window_secs).min(stored);
    let raw = (stored - open) * scheme.coded_overhead() + open * scheme.open_overhead();
    let volumes = ceil(stored / scheme.volume_bytes);
    Logical {
        blocks: ceil(stored / demand.avg_block_bytes),
        buckets: ceil(stored / scheme.bucket_bytes),
        volumes,
        fragments_per_volume: scheme.fragments_per_volume(),
        fragments: volumes * u64::from(scheme.fragments_per_volume()),
        fragment_bytes: scheme.volume_bytes / f64::from(scheme.k * scheme.region_data),
        open_bytes: open,
        raw_bytes: raw,
        overhead: if stored > 0.0 { raw / stored } else { 0.0 },
    }
}

fn gbps(bytes_per_year: f64) -> f64 {
    bytes_per_year * 8.0 / SECONDS_PER_YEAR / 1e9
}

fn on_prem(demand: &Demand, scheme: &Scheme, hw: &Hardware, logical: &Logical) -> OnPrem {
    let regions = scheme.regions();
    let needed_per_region = logical.raw_bytes * (1.0 + hw.spare_fraction) / f64::from(regions);
    // Every fragment of a volume needs its own server.
    let min_servers = u64::from(scheme.fragments_per_region());
    let servers_per_region = if needed_per_region > 0.0 {
        ceil(needed_per_region / hw.server_bytes()).max(min_servers)
    } else {
        0
    };
    let racks_per_region = servers_per_region.div_ceil(u64::from(hw.servers_per_rack));
    let cells_per_region = ceil(needed_per_region / hw.cell_raw_bytes);
    let servers = servers_per_region * u64::from(regions);
    let racks = racks_per_region * u64::from(regions);
    let installed = servers as f64 * hw.server_bytes();

    let servers_per_cell = ceil(hw.cell_raw_bytes / hw.server_bytes()).min(servers_per_region);
    let repair_hours_one_server = (servers_per_cell > 1).then(|| {
        let used = needed_per_region / servers_per_region as f64;
        let bits = f64::from(scheme.k) * used * 8.0;
        let bandwidth = (servers_per_cell - 1) as f64 * hw.server_nic_gbps * 1e9;
        bits / bandwidth / 3600.0
    });

    OnPrem {
        regions,
        raw_bytes: installed,
        drives: servers * u64::from(hw.drives_per_server),
        servers,
        racks,
        cells: cells_per_region * u64::from(regions),
        power_budget_kw: racks as f64 * hw.rack_kw,
        nic_gbps: servers as f64 * hw.server_nic_gbps,
        serving_gbps: gbps(demand.egress_bytes),
        standard_gbps: 30.0 * installed / PB,
        repair_hours_one_server,
    }
}

fn cloud(demand: &Demand, regions: u32) -> Cloud {
    let block = demand.avg_block_bytes;
    let gets = demand.egress_bytes / block;
    Cloud {
        regions,
        hot_bytes: demand.stored_bytes * demand.hot_fraction,
        cold_bytes: demand.stored_bytes * (1.0 - demand.hot_fraction),
        put_requests: demand.ingest_bytes / block,
        hot_get_requests: gets * (1.0 - demand.cold_read_fraction),
        cold_get_requests: gets * demand.cold_read_fraction,
        transitions: if demand.hot_fraction < 1.0 {
            demand.ingest_bytes / block
        } else {
            0.0
        },
        cold_retrieval_bytes: demand.egress_bytes * demand.cold_read_fraction,
        egress_bytes: demand.egress_bytes,
        inter_region_bytes: demand.ingest_bytes * f64::from(regions - 1),
    }
}

fn ec2(demand: &Demand, scheme: &Scheme, logical: &Logical, opts: &Ec2Options) -> Ec2 {
    let zones = scheme.zones_per_region;
    let node_bytes = opts.ebs_gib_per_node * GIB;
    // Each zone holds an equal share of every volume's fragments.
    let fragments_per_zone = scheme.fragments_per_region().div_ceil(zones);
    let nodes_per_zone = (ceil(logical.raw_bytes / f64::from(zones) / node_bytes) as u32)
        .max(opts.min_nodes_per_zone)
        .max(fragments_per_zone);

    let k = f64::from(scheme.k);
    let remote = f64::from(zones - 1) / f64::from(zones);
    let replicas_out = f64::from(scheme.open_replicas.saturating_sub(1));
    let fragments_out = f64::from(scheme.fragments_per_region()) * remote / k;
    let cross_zone_bytes =
        demand.ingest_bytes * (replicas_out + fragments_out) + demand.egress_bytes * remote;

    Ec2 {
        instance_type: opts.instance_type.clone(),
        zones,
        nodes_per_zone,
        nodes: nodes_per_zone * zones,
        ebs_gib_per_node: opts.ebs_gib_per_node,
        raw_bytes: logical.raw_bytes,
        cross_zone_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64) {
        let tolerance = expected.abs() * 1e-9 + 1e-9;
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn overhead_of_each_scheme() {
        close(Scheme::magic_pocket().coded_overhead(), 3.0);
        close(Scheme::magic_pocket_cold().coded_overhead(), 2.25);
        close(Scheme::satchel().coded_overhead(), 1.5);
        close(Scheme::satchel().open_overhead(), 3.0);
        close(Scheme::magic_pocket().open_overhead(), 8.0);
    }

    #[test]
    fn base_case_demand_matches_the_workload_model() {
        // Section 3 of #15, year 1.
        let d = Demand::base_case_year_one(500.0 * PB);
        close(d.stored_bytes, 550.0 * PB);
        close(d.ingest_bytes, 100.0 * PB);
        close(d.egress_bytes, 200.0 * PB);
        close(d.hot_fraction * 500.0, 83.333_333_333_333_33);
    }

    #[test]
    fn small_case_counts_by_hand() {
        // 1 TB of 2 MB blocks, all coded, Satchel layout.
        let demand = Demand {
            stored_bytes: TB,
            avg_block_bytes: 2e6,
            ingest_bytes: 0.0,
            egress_bytes: 0.0,
            hot_fraction: 1.0,
            cold_read_fraction: 0.0,
        };
        let l = size_logical(&demand, &Scheme::satchel());
        assert_eq!(l.blocks, 500_000);
        // 10^12 / 64 MiB = 14,901.16
        assert_eq!(l.volumes, 14_902);
        assert_eq!(l.buckets, 14_902);
        assert_eq!(l.fragments, 14_902 * 9);
        close(l.fragment_bytes, 64.0 * MIB / 6.0);
        close(l.raw_bytes, 1.5 * TB);
    }

    #[test]
    fn open_window_adds_replicated_data() {
        // 600 s of a year's ingest of 31.536 TB is 600 MB, stored 3x not 1.5x.
        let demand = Demand {
            stored_bytes: TB,
            avg_block_bytes: 2e6,
            ingest_bytes: SECONDS_PER_YEAR * 1e6,
            egress_bytes: 0.0,
            hot_fraction: 1.0,
            cold_read_fraction: 0.0,
        };
        let l = size_logical(&demand, &Scheme::satchel());
        close(l.open_bytes, 600.0 * 1e6);
        close(l.raw_bytes, (TB - 600e6) * 1.5 + 600e6 * 3.0);
    }

    #[test]
    fn dropbox_base_case() {
        let l = layout(&LayoutInput::dropbox_base_case()).unwrap();
        assert_eq!(l.logical.blocks, 275_000_000_000);
        close(l.logical.raw_bytes, 1650.0 * PB);

        // 825 PB per region on 3.2 PB servers: 257.8, so 258.
        let p = l.on_prem;
        assert_eq!(p.regions, 2);
        assert_eq!(p.servers, 516);
        assert_eq!(p.drives, 51_600);
        assert_eq!(p.racks, 66);
        assert_eq!(p.cells, 18);
        close(p.power_budget_kw, 990.0);
        close(p.serving_gbps, 200.0 * PB * 8.0 / SECONDS_PER_YEAR / 1e9);
        assert!(p.nic_gbps >= p.standard_gbps);
        // 6 x 3.198 PB read through 31 other servers at 200 Gbps: about 6.9 hours.
        let hours = p.repair_hours_one_server.unwrap();
        assert!((6.8..7.0).contains(&hours), "{hours}");

        let c = l.cloud;
        close(c.put_requests, 50e9);
        close(c.hot_get_requests + c.cold_get_requests, 100e9);
        close(c.cold_retrieval_bytes, 20.0 * PB);
        close(c.hot_bytes + c.cold_bytes, 550.0 * PB);
        close(c.inter_region_bytes, 0.0);
    }

    #[test]
    fn satchel_lab_fits_the_measured_cluster() {
        let l = layout(&LayoutInput::satchel_lab()).unwrap();
        assert_eq!(l.ec2.nodes_per_zone, 3);
        assert_eq!(l.ec2.nodes, 9);
        assert_eq!(l.ec2.instance_type, "m7g.medium");
        assert!(l.ec2.raw_bytes < 9.0 * 50.0 * GIB);
        // Put: 2 replicas + 6 of 9 fragments / 6; get: 2 of 3 reads remote.
        let d = LayoutInput::satchel_lab().demand;
        close(
            l.ec2.cross_zone_bytes,
            d.ingest_bytes * 3.0 + d.egress_bytes * 2.0 / 3.0,
        );
    }

    #[test]
    fn rounding_never_drops_below_the_placement() {
        let mut input = LayoutInput::dropbox_base_case();
        input.demand.stored_bytes = 1.0;
        let l = layout(&input).unwrap();
        assert_eq!(l.logical.volumes, 1);
        assert!(l.logical.fragments >= u64::from(input.scheme.k + input.scheme.m));
        assert_eq!(l.on_prem.servers, 9 * 2);
        assert!(l.ec2.nodes_per_zone >= 3);
    }

    #[test]
    fn empty_workload_needs_no_hardware() {
        let mut input = LayoutInput::dropbox_base_case();
        input.demand.stored_bytes = 0.0;
        input.demand.ingest_bytes = 0.0;
        let l = layout(&input).unwrap();
        assert_eq!(l.logical.volumes, 0);
        assert_eq!(l.on_prem.servers, 0);
        assert_eq!(l.on_prem.repair_hours_one_server, None);
    }

    #[test]
    fn second_cloud_region_copies_ingest() {
        let mut input = LayoutInput::dropbox_base_case();
        input.cloud_regions = 2;
        let c = layout(&input).unwrap().cloud;
        close(c.inter_region_bytes, 100.0 * PB);
    }

    #[test]
    fn spares_add_capacity() {
        let mut input = LayoutInput::dropbox_base_case();
        let without = layout(&input).unwrap().on_prem.servers;
        input.hardware.spare_fraction = 0.1;
        assert!(layout(&input).unwrap().on_prem.servers > without);
    }

    #[test]
    fn rejects_bad_inputs() {
        let base = LayoutInput::dropbox_base_case();
        type BreakIt = fn(&mut LayoutInput);
        let cases: [(&str, BreakIt); 5] = [
            ("avg_block_bytes", |i| i.demand.avg_block_bytes = 0.0),
            ("hot_fraction", |i| i.demand.hot_fraction = 1.5),
            ("k", |i| i.scheme.k = 0),
            ("stored_bytes", |i| i.demand.stored_bytes = f64::NAN),
            ("cloud_regions", |i| i.cloud_regions = 0),
        ];
        for (name, break_it) in cases {
            let mut input = base.clone();
            break_it(&mut input);
            let err = layout(&input).unwrap_err();
            assert!(err.contains(name), "{name}: {err}");
        }
    }

    #[test]
    fn layout_round_trips_through_json() {
        let l = layout(&LayoutInput::dropbox_base_case()).unwrap();
        let json = serde_json::to_string(&l).unwrap();
        assert_eq!(serde_json::from_str::<Layout>(&json).unwrap(), l);

        let input = LayoutInput::satchel_lab();
        let json = serde_json::to_string(&input).unwrap();
        assert_eq!(serde_json::from_str::<LayoutInput>(&json).unwrap(), input);
    }
}
