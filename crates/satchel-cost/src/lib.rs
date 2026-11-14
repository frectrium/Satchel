//! Cost model for the Satchel case workload.
//!
//! [`layout`] sizes a workload: logical units, on-prem hardware, cloud storage
//! tiers and the Satchel-on-EC2 cluster. [`pricing`] prices that layout year
//! by year with the TOML tables in [`prices`] (#52), following
//! `docs/cost/method.md`, and [`compare`] puts the providers side by side and
//! finds the break-even data volume (#26). The calculator page (#54) uses the
//! same code. The library does no I/O, so it also builds for WASM.

pub mod compare;
pub mod layout;
pub mod prices;
pub mod pricing;
pub mod workload;
