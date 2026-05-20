//! Visitor utilities for traversing parsed syntax trees.
//!
//! This module provides the [`crate::visitor::Visitor`] trait for depth-first
//! traversal and several ready-made walkers and visitors:
//!
//! - [`crate::visitor::TreeWalker`] — depth-first traversal
//! - [`crate::visitor::BreadthFirstWalker`] — breadth-first, level-order traversal
//! - [`crate::visitor::TransformWalker`] / [`crate::visitor::TransformVisitor`] — bottom-up tree
//!   transformation
//! - [`crate::visitor::StatsVisitor`] — collects node counts and tree depth
//! - [`crate::visitor::SearchVisitor`] — finds nodes matching a predicate
//! - [`crate::visitor::PrettyPrintVisitor`] — produces an indented text representation
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
