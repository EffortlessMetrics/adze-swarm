//! Execute the canonical optional-origin program over actual selected nodes.
use super::{AdzeDocument, NodeId, ParseOutcome};
use crate::generated_schema::{GeneratedFieldProjectionV1, GeneratedOwnerProjection};
use std::collections::{BTreeMap, BTreeSet};

/// A checked selected field path is unavailable or incomplete.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum FieldProjectionError {
    /// This entry point supplied facts only, or the selected owner is unsupported.
    #[error("canonical field projection unavailable for node {node:?}")]
    Unavailable {
        /// Selected document node.
        node: NodeId,
    },
    /// Selected structure or recovery facts cannot establish the declared path.
    #[error("canonical field projection incomplete for node {node:?}")]
    Incomplete {
        /// Selected document node.
        node: NodeId,
    },
}
#[derive(Clone, Debug, Default)]
pub(super) struct ProjectionIndex {
    available: bool,
    fields: BTreeMap<NodeId, Result<BTreeMap<String, Vec<NodeId>>, FieldProjectionError>>,
}
/// Borrowed capability for a node whose selected fields and targets were checked.
///
/// This view references existing document-local NodeIds; it owns no syntax tree.
#[derive(Clone, Copy, Debug)]
pub struct FieldProjectionView<'doc> {
    document: &'doc AdzeDocument,
    id: NodeId,
}
impl<'doc> FieldProjectionView<'doc> {
    /// Return actual selected targets for a declared field of this checked view.
    ///
    /// Availability was established by `field_projection_view`, including all
    /// selected targets transitively. An undeclared field name yields no nodes;
    /// generated getters use only the names required by their checked cast.
    pub fn nodes(&self, field: &str) -> impl Iterator<Item = ProjectedTarget<'doc>> + 'doc {
        let document = self.document;
        document
            .projection
            .fields
            .get(&self.id)
            .and_then(|result| result.as_ref().ok())
            .and_then(|fields| fields.get(field))
            .into_iter()
            .flatten()
            .copied()
            .map(move |id| ProjectedTarget {
                view: Self { document, id },
            })
    }
}
/// An actual selected target yielded by a checked field projection.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedTarget<'doc> {
    view: FieldProjectionView<'doc>,
}
impl<'doc> ProjectedTarget<'doc> {
    /// Existing native document-local identity.
    pub fn node_id(&self) -> NodeId {
        self.view.id
    }
    /// Original grammar identity, independent of visible aliases.
    pub fn grammar_id(&self) -> Option<adze_ir::SymbolId> {
        self.view
            .document
            .tree()
            .node(self.view.id)
            .map(|node| node.grammar_id())
    }
    /// Transitive capability checked by the originating view constructor.
    pub fn view(&self) -> FieldProjectionView<'doc> {
        self.view
    }
}
impl AdzeDocument {
    pub(crate) fn bind_field_projection(
        &mut self,
        program: Option<&'static GeneratedFieldProjectionV1>,
    ) {
        let Some(program) = program else {
            return;
        };
        let mut index = ProjectionIndex {
            available: true,
            fields: BTreeMap::new(),
        };
        for node in self
            .syntax
            .nodes
            .iter()
            .enumerate()
            .map(|(i, _)| NodeId::new(i))
        {
            let Some(record) = self.tree().node(node) else {
                continue;
            };
            if self
                .language
                .field_semantics
                .get(&record.grammar_id())
                .is_none_or(BTreeMap::is_empty)
            {
                continue;
            }
            let result = program
                .owners
                .iter()
                .find(|owner| owner.owner == record.grammar_id().0)
                .ok_or(FieldProjectionError::Unavailable { node })
                .and_then(|owner| project_owner(self, node, owner));
            index.fields.insert(node, result);
        }
        self.projection = index;
    }
    /// Obtain a checked logical field view for a selected node.
    ///
    /// Validate this owner and its selected targets transitively. Legacy fact-only
    /// documents cannot grant field capability. Nodes with no declared fields may
    /// still cast without unrelated projection support.
    pub fn field_projection_view(
        &self,
        id: NodeId,
        required_fields: &[&str],
    ) -> Result<FieldProjectionView<'_>, FieldProjectionError> {
        check_node(self, id, required_fields)?;
        Ok(FieldProjectionView { document: self, id })
    }
}
fn check_node(
    document: &AdzeDocument,
    root: NodeId,
    required: &[&str],
) -> Result<(), FieldProjectionError> {
    // Selected syntax may be deeply nested. Validate transitively without a
    // recursive Rust call stack, and distinguish a shared target from a cycle.
    let mut pending = vec![(root, false)];
    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    while let Some((id, exiting)) = pending.pop() {
        if exiting {
            active.remove(&id);
            complete.insert(id);
            continue;
        }
        if complete.contains(&id) {
            continue;
        }
        if !active.insert(id) {
            return Err(FieldProjectionError::Incomplete { node: id });
        }
        let node = document
            .tree()
            .node(id)
            .ok_or(FieldProjectionError::Incomplete { node: id })?;
        let facts = document.language.field_semantics.get(&node.grammar_id());
        let required = if id == root { required } else { &[] };
        if required.is_empty() && facts.is_none_or(BTreeMap::is_empty) {
            active.remove(&id);
            complete.insert(id);
            continue;
        }
        if !document.projection.available {
            return Err(FieldProjectionError::Unavailable { node: id });
        }
        let fields = document
            .projection
            .fields
            .get(&id)
            .ok_or(FieldProjectionError::Unavailable { node: id })?
            .as_ref()
            .map_err(Clone::clone)?;
        if !required.iter().all(|name| fields.contains_key(*name)) {
            return Err(FieldProjectionError::Incomplete { node: id });
        }
        pending.push((id, true));
        pending.extend(
            fields
                .values()
                .flatten()
                .rev()
                .map(|target| (*target, false)),
        );
    }
    Ok(())
}

fn project_owner(
    document: &AdzeDocument,
    id: NodeId,
    owner: &GeneratedOwnerProjection,
) -> Result<BTreeMap<String, Vec<NodeId>>, FieldProjectionError> {
    let incomplete = || FieldProjectionError::Incomplete { node: id };
    if document.outcome() != ParseOutcome::Complete {
        return Err(incomplete());
    }
    let node = document.tree().node(id).ok_or_else(incomplete)?;
    let children = node
        .child_edges()
        .map(|edge| edge.child().ok_or_else(incomplete))
        .collect::<Result<Vec<_>, _>>()?;
    let children: Vec<_> = children
        .into_iter()
        .filter(|child| !child.is_extra())
        .collect();
    if children.len() != owner.rhs.len() {
        return Err(incomplete());
    }
    // Every slot must match its declared original identity or the explicitly
    // sanctioned unary elision of the optional carrier in that exact slot.
    for (position, child) in children.iter().enumerate() {
        let expected = owner.rhs[position];
        if child.grammar_id().0 != expected
            && !owner.fields.iter().any(|field| {
                field.position == position
                    && field
                        .optional
                        .is_some_and(|optional| optional.helper == expected)
                    && child.grammar_id().0 == field.target
            })
        {
            return Err(incomplete());
        }
    }
    let mut fields: BTreeMap<String, Vec<NodeId>> = BTreeMap::new();
    for field in owner.fields {
        let targets = fields.entry(field.name.to_owned()).or_default();
        let selected = children.get(field.position).ok_or_else(incomplete)?;
        if let Some(optional) = field.optional {
            if selected.grammar_id().0 == optional.helper {
                let selected_children = selected
                    .child_edges()
                    .map(|edge| edge.child().ok_or_else(incomplete))
                    .collect::<Result<Vec<_>, _>>()?;
                let selected_children: Vec<_> = selected_children
                    .into_iter()
                    .filter(|child| !child.is_extra())
                    .collect();
                match selected_children.as_slice() {
                    [] => {}
                    [child] if child.grammar_id().0 == field.target => {
                        targets.push(child.node_id())
                    }
                    _ => return Err(incomplete()),
                }
            } else if selected.grammar_id().0 == field.target {
                targets.push(selected.node_id());
            } else {
                return Err(incomplete());
            }
        } else if selected.grammar_id().0 == field.target {
            targets.push(selected.node_id());
        } else {
            return Err(incomplete());
        }
    }
    Ok(fields)
}
