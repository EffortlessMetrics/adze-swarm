//! Field-aware tree structures where field names are stored as edge properties.
//! This design correctly models that field-ness is a property of parent→child relationships.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

mod language;
mod node;
mod reduction;
mod types;

pub use language::TSLanguage;

/// Friendly name for [`TSLanguage`]; both names denote the same type.
pub use language::TSLanguage as Language;
pub use node::{ParsedChild, ParsedNode};
pub use types::Point;
