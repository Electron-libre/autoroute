//! Specification as a single source of truth for your rust http/json API.
//!
//! This crate is a facade: the procedural macros live in `autoroute-macros`
//! and are re-exported here, so that runtime support (e.g. request
//! validation) can be shipped alongside them.

pub use autoroute_macros::{gen_config_from, gen_config_from_path};

#[cfg(feature = "validation")]
pub mod validation;
