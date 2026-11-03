//! Execute the canonical optional-origin program over actual selected nodes.
use super::{AdzeDocument, NodeId, ParseOutcome};
use crate::generated_schema::GeneratedFieldProjectionV1;
use adze_tablegen::field_projection::{FieldOccurrence, OptionalCarrier, OwnerProjection};
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
    /// A concrete selected field target violates the canonical target set.
    #[error("wrong target {target:?} for field {field} on {node:?}")]
    WrongTarget {
        /// Field owner.
        node: NodeId,
        /// Canonical field name.
        field: String,
        /// Actual document-local offending target.
        target: NodeId,
        /// Actual grammar identity.
        actual: adze_ir::SymbolId,
    },
    /// A required field has no selected occurrence.
    #[error("missing required field {field} on {node:?}")]
    MissingRequired {
        /// Field owner.
        node: NodeId,
        /// Canonical field name.
        field: String,
    },
    /// A singular field has multiple actual selected occurrences.
    #[error("too many targets for field {field} on {node:?}")]
    TooMany {
        /// Field owner.
        node: NodeId,
        /// Canonical field name.
        field: String,
        /// Every actual occurrence, in source order.
        targets: Vec<NodeId>,
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
        let owners: Vec<_> = program
            .owners
            .iter()
            .map(|owner| OwnerProjection {
                owner: adze_ir::SymbolId(owner.owner),
                rhs: owner.rhs.iter().map(|id| adze_ir::SymbolId(*id)).collect(),
                fields: owner
                    .fields
                    .iter()
                    .map(|field| FieldOccurrence {
                        name: field.name.to_owned(),
                        position: field.position,
                        target: adze_ir::SymbolId(field.target),
                        optional: field.optional.map(|optional| OptionalCarrier {
                            helper: adze_ir::SymbolId(optional.helper),
                            present_alternative: optional.present_alternative,
                            empty_alternative: optional.empty_alternative,
                        }),
                    })
                    .collect(),
            })
            .collect();
        self.bind_projection_owners(&owners, program.projection_revision >= 2);
    }

    pub(crate) fn bind_complete_ir_projection(&mut self, grammar: &adze_ir::Grammar) {
        let owners = adze_tablegen::field_projection::for_grammar(grammar);
        self.bind_projection_owners(&owners, true);
    }

    fn bind_projection_owners(&mut self, owners: &[OwnerProjection], direct_certified: bool) {
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
            let result = owners
                .iter()
                .find(|owner| owner.owner == record.grammar_id())
                .ok_or(FieldProjectionError::Unavailable { node })
                .and_then(|owner| project_owner(self, node, owner, direct_certified));
            index.fields.insert(node, result);
        }
        self.projection = index;
    }
    /// Strictly validate fields on one owner using the producer's shared projection.
    ///
    /// This is node-scoped: target nodes retain their own separate validation
    /// obligation. Legacy `validate_fields` continues grant-only validation.
    pub fn validate_fields_strict_at(&self, node: NodeId) -> Result<(), FieldProjectionError> {
        if !self.projection.available {
            return Err(FieldProjectionError::Unavailable { node });
        }
        self.projection
            .fields
            .get(&node)
            .ok_or(FieldProjectionError::Unavailable { node })?
            .as_ref()
            .map(|_| ())
            .map_err(Clone::clone)
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
    owner: &OwnerProjection,
    direct_certified: bool,
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
    if direct_certified && owner.fields.iter().all(|field| field.optional.is_none()) {
        // Revision 2 certifies direct FIELD occurrences only for these admitted
        // single-rule, leaf-RHS owners. Do not search descendants or infer
        // transparency from names. Preserve every actual NodeId for diagnostics.
        let mut fields: BTreeMap<String, Vec<NodeId>> = owner
            .fields
            .iter()
            .map(|field| (field.name.clone(), Vec::new()))
            .collect();
        for edge in node.child_edges() {
            let child = edge.child().ok_or_else(incomplete)?;
            if child.is_extra() {
                continue;
            }
            if child.is_error() || child.is_missing() {
                return Err(incomplete());
            }
            if let Some(name) = edge.field_name() {
                let targets = fields.get_mut(name).ok_or_else(incomplete)?;
                targets.push(child.node_id());
            }
        }
        validate_observed_fields(document, id, &fields)?;
        // Semantic errors take precedence above; a valid grant alone does not
        // establish a positional producer path for checked typed handles.
        if children.len() != owner.rhs.len()
            || children
                .iter()
                .zip(&owner.rhs)
                .any(|(child, expected)| child.grammar_id() != *expected)
        {
            return Err(incomplete());
        }
        for field in &owner.fields {
            if node
                .child_edges()
                .filter(|edge| edge.child().is_some_and(|child| !child.is_extra()))
                .nth(field.position)
                .and_then(|edge| edge.field_name())
                != Some(field.name.as_str())
            {
                return Err(incomplete());
            }
        }
        return Ok(fields);
    }
    if children.len() != owner.rhs.len() {
        return Err(incomplete());
    }
    // Every slot must match its declared original identity or the explicitly
    // sanctioned unary elision of the optional carrier in that exact slot.
    for (position, child) in children.iter().enumerate() {
        let expected = owner.rhs[position].0;
        if child.grammar_id().0 != expected
            && !owner.fields.iter().any(|field| {
                field.position == position
                    && field
                        .optional
                        .as_ref()
                        .is_some_and(|optional| optional.helper.0 == expected)
                    && child.grammar_id() == field.target
            })
        {
            return Err(incomplete());
        }
    }
    let mut fields: BTreeMap<String, Vec<NodeId>> = BTreeMap::new();
    for field in &owner.fields {
        let targets = fields.entry(field.name.to_owned()).or_default();
        let selected = children.get(field.position).ok_or_else(incomplete)?;
        if let Some(optional) = field.optional.as_ref() {
            if selected.grammar_id() == optional.helper {
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
                    [child] if child.grammar_id() == field.target => targets.push(child.node_id()),
                    _ => return Err(incomplete()),
                }
            } else if selected.grammar_id() == field.target {
                targets.push(selected.node_id());
            } else {
                return Err(incomplete());
            }
        } else if selected.grammar_id() == field.target {
            targets.push(selected.node_id());
        } else {
            return Err(incomplete());
        }
    }
    validate_observed_fields(document, id, &fields)?;
    Ok(fields)
}

fn validate_observed_fields(
    document: &AdzeDocument,
    node: NodeId,
    fields: &BTreeMap<String, Vec<NodeId>>,
) -> Result<(), FieldProjectionError> {
    let owner = document
        .tree()
        .node(node)
        .ok_or(FieldProjectionError::Incomplete { node })?;
    let facts = document
        .language
        .field_semantics
        .get(&owner.grammar_id())
        .ok_or(FieldProjectionError::Unavailable { node })?;
    for (name, fact) in facts {
        let targets = fields
            .get(name)
            .ok_or(FieldProjectionError::Incomplete { node })?;
        for target in targets {
            let actual = document
                .tree()
                .node(*target)
                .ok_or(FieldProjectionError::Incomplete { node })?
                .grammar_id();
            if !fact.target_kinds.contains(&actual) {
                return Err(FieldProjectionError::WrongTarget {
                    node,
                    field: name.clone(),
                    target: *target,
                    actual,
                });
            }
        }
        if fact.required && targets.is_empty() {
            return Err(FieldProjectionError::MissingRequired {
                node,
                field: name.clone(),
            });
        }
        if !fact.multiple && targets.len() > 1 {
            return Err(FieldProjectionError::TooMany {
                node,
                field: name.clone(),
                targets: targets.clone(),
            });
        }
    }
    Ok(())
}
