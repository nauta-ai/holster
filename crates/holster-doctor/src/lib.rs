//! Holster Doctor scanner library.
//!
//! This crate owns the platform-pure Doctor logic shared by the Tauri
//! desktop app and the standalone `holster-doctor` CLI.

pub mod agent_profiles;
pub mod detectors;
pub mod env_example;
pub mod gitignore;
pub mod preflight;
pub mod scanner;
