//! Query engine components (AST, compiler, matcher, highlighter, etc.).
//!
//! Three same-named type families coexist here by design and are tracked by
//! #871 §d — they are distinct types, not aliases, and must not be unified
//! outside #861:
//!
//! - `matcher::{QueryMatch, QueryCapture, QueryMatcher, QueryMatches}` and
//!   `cursor::QueryCursor`: the source-free `ParseNode` matcher, re-exported
//!   at this module's root.
//! - `matcher_v2::{QueryMatch, QueryCapture, QueryMatcher, QueryMatches}`:
//!   the source-aware matcher, including the document-backed
//!   `DocumentQueryMatch` path (available with the `query` feature) that #861
//!   consolidates on as the recommended primary.
//! - [`crate::glr_query`]: GLR-forest-specific `QueryMatch`/`QueryCapture`/
//!   `QueryCursor` over [`Subtree`](crate::glr_query::Subtree).
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

// Tree-sitter query language support for adze
// This module implements the S-expression based query language for pattern matching

pub mod ast;
pub mod compiler;
pub mod cursor;
pub mod highlights;
pub mod matcher;
pub mod matcher_v2;
pub mod parser;
pub mod pattern;
pub mod predicate_eval;

pub use ast::{Query, QueryError};
pub use compiler::compile_query;
pub use cursor::QueryCursor;
pub use highlights::{Color, Highlight, Highlighter, Theme};
pub use matcher::{QueryCapture, QueryMatch, QueryMatches};
pub use pattern::{Pattern, Predicate};
pub use predicate_eval::PredicateContext;
