macro_rules! imported {
    ($module:ident, $file:literal) => {
        mod $module {
            include!(concat!(env!("OUT_DIR"), "/", $file));
        }
    };
}
imported!(pattern_choice, "imported_pattern_choice.rs");
imported!(pattern_sequence, "imported_pattern_sequence.rs");
imported!(literal_choice, "imported_literal_choice.rs");
imported!(literal_sequence, "imported_literal_sequence.rs");
#[cfg(test)]
mod tests {
    use super::*;
    use adze::document::{AdzeDocument, NodeId, ParseOutcome};
    fn assert_leaf(document: &AdzeDocument, source: &str, binding: u16, wrapper: u16, start: u16) {
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        assert_eq!(document.tree().root().byte_range(), 0..source.len());
        let nodes: Vec<_> = (0..document.tree().node_count())
            .map(NodeId::new)
            .map(|id| document.tree().node(id).unwrap())
            .collect();
        // This control isolates the imported-reference lowering at the
        // declared semantic owner. The independently retained global root
        // identity red is not relaxed by this additional scoped oracle.
        let owners: Vec<_> = nodes
            .iter()
            .filter(|node| node.grammar_id().0 == start)
            .collect();
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].byte_range(), 0..source.len());
        assert_eq!(owners[0].child_count(), 1);
        assert_ne!(owners[0].child(0).unwrap().grammar_id().0, wrapper);
        let selected: Vec<_> = nodes
            .iter()
            .filter(|node| node.grammar_id().0 == binding)
            .collect();
        assert_eq!(selected.len(), 1);
        assert!(selected[0].flags().is_terminal());
        assert_eq!(selected[0].utf8_text().unwrap(), source);
        assert_eq!(selected[0].byte_range(), 0..source.len());
        assert_eq!(owners[0].child(0).unwrap().node_id(), selected[0].node_id());
    }
    fn assert_other_choice(document: &AdzeDocument, binding: u16, wrapper: u16, start: u16) {
        assert_eq!(document.outcome(), ParseOutcome::Complete);
        assert!(document.diagnostics().is_empty());
        let owner = (0..document.tree().node_count())
            .map(NodeId::new)
            .map(|id| document.tree().node(id).unwrap())
            .find(|node| node.grammar_id().0 == start)
            .unwrap();
        let child = owner.child(0).unwrap();
        assert!(child.flags().is_terminal());
        assert_eq!(child.utf8_text().unwrap(), "!");
        assert_eq!(child.byte_range(), 0..1);
        assert_ne!(child.grammar_id().0, binding);
        assert_ne!(child.grammar_id().0, wrapper);
    }
    #[test]
    fn test_imported_pattern_choice_preserves_direct_actual_token() {
        assert_leaf(
            &pattern_choice::parse_document(pattern_choice::SOURCE).unwrap(),
            pattern_choice::SOURCE,
            pattern_choice::BINDING,
            pattern_choice::WRAPPER,
            pattern_choice::START,
        );
        assert_other_choice(
            &pattern_choice::parse_document("!").unwrap(),
            pattern_choice::BINDING,
            pattern_choice::WRAPPER,
            pattern_choice::START,
        );
    }
    #[test]
    fn test_imported_pattern_sequence_preserves_direct_actual_token() {
        assert_leaf(
            &pattern_sequence::parse_document(pattern_sequence::SOURCE).unwrap(),
            pattern_sequence::SOURCE,
            pattern_sequence::BINDING,
            pattern_sequence::WRAPPER,
            pattern_sequence::START,
        );
    }
    #[test]
    fn test_imported_literal_choice_preserves_direct_actual_token() {
        assert_leaf(
            &literal_choice::parse_document(literal_choice::SOURCE).unwrap(),
            literal_choice::SOURCE,
            literal_choice::BINDING,
            literal_choice::WRAPPER,
            literal_choice::START,
        );
        assert_other_choice(
            &literal_choice::parse_document("!").unwrap(),
            literal_choice::BINDING,
            literal_choice::WRAPPER,
            literal_choice::START,
        );
    }
    #[test]
    fn test_imported_literal_sequence_preserves_direct_actual_token() {
        assert_leaf(
            &literal_sequence::parse_document(literal_sequence::SOURCE).unwrap(),
            literal_sequence::SOURCE,
            literal_sequence::BINDING,
            literal_sequence::WRAPPER,
            literal_sequence::START,
        );
    }
}
