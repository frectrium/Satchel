//! The case workload (#15), year by year. Decimal bytes throughout.

use serde::{Deserialize, Serialize};

use crate::layout::{Demand, PB};

/// Every parameter of the workload model (#15). [`Workload::base`] is its
/// base case.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Workload {
    pub start_bytes: f64,
    /// Yearly growth of stored data. Ingest equals growth (no deletes).
    pub growth: f64,
    pub years: u32,
    pub avg_block_bytes: f64,
    /// Egress as a multiple of ingest.
    pub egress_ratio: f64,
    /// Share of reads that hit data older than 12 months.
    pub cold_read_fraction: f64,
}

impl Workload {
    /// 500 PB [B1], 20% growth [T5], 5 years, 2 MB blocks and egress of
    /// twice the ingest (assumptions in #15), 10% of reads on cold data [B5].
    pub fn base() -> Self {
        Self {
            start_bytes: 500.0 * PB,
            growth: 0.20,
            years: 5,
            avg_block_bytes: 2e6,
            egress_ratio: 2.0,
            cold_read_fraction: 0.10,
        }
    }

    pub fn with_start(self, start_bytes: f64) -> Self {
        Self {
            start_bytes,
            ..self
        }
    }

    pub fn year(&self, year: u32) -> Year {
        let g = self.growth;
        let start = self.start_bytes * (1.0 + g).powi(year as i32 - 1);
        let end = start * (1.0 + g);
        let ingest = end - start;
        Year {
            year,
            start_bytes: start,
            end_bytes: end,
            avg_bytes: (start + end) / 2.0,
            ingest_bytes: ingest,
            egress_bytes: ingest * self.egress_ratio,
            // Data written in the last 12 months is g / (1 + g) of the total (#15).
            hot_fraction: g / (1.0 + g),
            // Last year's ingest moves to the cold tier this year (#15).
            transition_bytes: start * g / (1.0 + g),
            avg_block_bytes: self.avg_block_bytes,
            cold_read_fraction: self.cold_read_fraction,
        }
    }

    pub fn years(&self) -> Vec<Year> {
        (1..=self.years).map(|y| self.year(y)).collect()
    }
}

/// One year of the workload.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Year {
    pub year: u32,
    pub start_bytes: f64,
    pub end_bytes: f64,
    /// What cloud storage bills for.
    pub avg_bytes: f64,
    pub ingest_bytes: f64,
    pub egress_bytes: f64,
    pub hot_fraction: f64,
    pub transition_bytes: f64,
    pub avg_block_bytes: f64,
    pub cold_read_fraction: f64,
}

impl Year {
    pub fn puts(&self) -> f64 {
        self.ingest_bytes / self.avg_block_bytes
    }

    pub fn gets(&self) -> f64 {
        self.egress_bytes / self.avg_block_bytes
    }

    pub fn transitions(&self) -> f64 {
        self.transition_bytes / self.avg_block_bytes
    }

    /// Demand for the sizing model (#53). Hardware has to hold the data at
    /// the end of the year, so `stored_bytes` is the year-end figure here.
    pub fn demand_at_end(&self) -> Demand {
        Demand {
            stored_bytes: self.end_bytes,
            avg_block_bytes: self.avg_block_bytes,
            ingest_bytes: self.ingest_bytes,
            egress_bytes: self.egress_bytes,
            hot_fraction: self.hot_fraction,
            cold_read_fraction: self.cold_read_fraction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= expected.abs() * 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn base_case_matches_section_3_of_the_workload_model() {
        let w = Workload::base();
        let y1 = w.year(1);
        close(y1.end_bytes, 600.0 * PB);
        close(y1.avg_bytes, 550.0 * PB);
        close(y1.ingest_bytes, 100.0 * PB);
        close(y1.egress_bytes, 200.0 * PB);
        close(y1.puts(), 50e9);
        close(y1.gets(), 100e9);
        // 83.3 PB was written in the year before the baseline.
        close(y1.transition_bytes, 500.0 * PB / 6.0);
        let y5 = w.year(5);
        close(y5.end_bytes, 500.0 * PB * 1.2_f64.powi(5));
        let ingest: f64 = w.years().iter().map(|y| y.ingest_bytes).sum();
        // 744.2 PB over 5 years.
        assert!((ingest / PB - 744.2).abs() < 0.05, "{}", ingest / PB);
    }
}
