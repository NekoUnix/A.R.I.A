//! ARIA's independently authored, Rust-native model runtime.
//!
//! This crate deliberately contains no vendored C runtime or copied evaluator.
//! Until format decoding and deformation pass the local-model parity gate, the
//! desktop application continues to use its existing production runtime.

pub mod draw_order;
pub mod geometry;
pub mod glue_schedule;
pub mod gpu_blend_plan;
pub mod gpu_key_plan;
pub mod moc;
pub mod resident;
pub mod rig;
