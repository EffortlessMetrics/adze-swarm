use crate::parser_v4::ParseNode;

use super::CursorFrame;

pub(super) fn goto_next_sibling<'a>(
    parents: &mut [CursorFrame<'a>],
    current: &mut &'a ParseNode,
) -> bool {
    let Some(parent) = parents.last_mut() else {
        return false;
    };

    let next_index = parent.child_index + 1;
    let Some(next) = parent.node.children.get(next_index) else {
        return false;
    };

    parent.child_index = next_index;
    *current = next;
    true
}

pub(super) fn goto_previous_sibling<'a>(
    parents: &mut [CursorFrame<'a>],
    current: &mut &'a ParseNode,
) -> bool {
    let Some(parent) = parents.last_mut() else {
        return false;
    };
    let Some(previous_index) = parent.child_index.checked_sub(1) else {
        return false;
    };
    let Some(previous) = parent.node.children.get(previous_index) else {
        return false;
    };

    parent.child_index = previous_index;
    *current = previous;
    true
}
