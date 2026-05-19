//! Core line/column byte-position tracking utilities.
//!
//! The tracker is byte-oriented and supports `\n`, `\r`, and `\r\n` line endings.

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]
#![cfg_attr(feature = "strict_api", deny(unreachable_pub))]
#![cfg_attr(not(feature = "strict_api"), warn(unreachable_pub))]
#![cfg_attr(feature = "strict_docs", deny(missing_docs))]
#![cfg_attr(not(feature = "strict_docs"), allow(missing_docs))]

mod scanner;
mod tracker;

pub use tracker::LineCol;

#[cfg(test)]
mod tests;
