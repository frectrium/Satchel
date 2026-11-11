//! Cost model for the Satchel case workload.
//!
//! [`layout`] sizes a workload: logical units, on-prem hardware, cloud storage
//! tiers and the Satchel-on-EC2 cluster. The pricing engine (#52) prices that
//! layout and the calculator page (#54) draws it. The crate does no I/O, so it
//! also builds for WASM.

pub mod layout;
