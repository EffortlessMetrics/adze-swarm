//! Visitor utilities for traversing parsed syntax trees.
//!
//! This module provides the [`Visitor`] trait for depth-first traversal and
//! several ready-made walkers and visitors:
//!
//! - [`TreeWalker`] — depth-first traversal
//! - [`BreadthFirstWalker`] — breadth-first, level-order traversal
//! - [`TransformWalker`] / [`TransformVisitor`] — bottom-up tree
//!   transformation
//! - [`StatsVisitor`] — collects node counts and tree depth
//! - [`SearchVisitor`] — finds nodes matching a predicate
//! - [`PrettyPrintVisitor`] — produces an indented text representation
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

#[cfg(feature = "pure-rust")]
use crate::pure_parser::ParsedNode as Node;
#[cfg(not(feature = "pure-rust"))]
use crate::tree_sitter::Node;

mod api;
mod transform;
mod visitors;
mod walkers;

pub use api::{Visitor, VisitorAction};
pub use transform::{TransformVisitor, TransformWalker};
pub use visitors::{PrettyPrintVisitor, SearchVisitor, StatsVisitor};
pub use walkers::{BreadthFirstWalker, TreeWalker};

#[cfg(test)]
mod tests;
