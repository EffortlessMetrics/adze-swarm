use crate::document::NodeId;

use super::{CursorFrame, Tree};

pub(super) fn goto_next_sibling(
    tree: &Tree,
    parents: &mut [CursorFrame],
    current: &mut NodeId,
) -> bool {
    let Some(parent) = parents.last_mut() else {
        return false;
    };

    let syntax = tree.core.document.syntax();
    let next_index = parent.child_index + 1;
    let Some(next) = syntax.child_id(parent.node, next_index) else {
        return false;
    };

    parent.child_index = next_index;
    *current = next;
    true
}

pub(super) fn goto_previous_sibling(
    tree: &Tree,
    parents: &mut [CursorFrame],
    current: &mut NodeId,
) -> bool {
    let Some(parent) = parents.last_mut() else {
        return false;
    };
    let Some(previous_index) = parent.child_index.checked_sub(1) else {
        return false;
    };
    let syntax = tree.core.document.syntax();
    let Some(previous) = syntax.child_id(parent.node, previous_index) else {
        return false;
    };

    parent.child_index = previous_index;
    *current = previous;
    true
}
