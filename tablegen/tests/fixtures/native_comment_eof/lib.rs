include!("../grammar.rs");

#[cfg(test)]
mod tests {
    use super::grammar;

    #[test]
    fn test_native_eof_comment_uses_logical_source_and_retains_nul() {
        for source in ["17# no final newline", "17# before\0after"] {
            assert_eq!(grammar::parse(source).unwrap().value, 17);
            let doc = grammar::parse_document(source).unwrap();
            assert!(
                doc.diagnostics().is_empty(),
                "{source:?}: {:?}",
                doc.diagnostics()
            );
            assert_eq!(doc.metadata().error_count, 0);
            assert_eq!(doc.tree().root().byte_range(), 0..source.len());
            assert_eq!(doc.ast::<grammar::Number>().unwrap().value, 17);
            let mut found_comment = false;
            for index in 0..doc.tree().node_count() {
                let node = doc.tree().node(adze::document::NodeId::new(index)).unwrap();
                assert!(node.byte_range().end <= source.len());
                if node.is_extra() && node.byte_range() == (2..source.len()) {
                    assert_eq!(node.utf8_text().unwrap(), &source[2..]);
                    assert!(!node.is_error());
                    assert!(!node.is_missing());
                    found_comment = true;
                }
            }
            assert!(
                found_comment,
                "the actual EOF comment must retain its complete source range"
            );
            println!(
                "native_comment={source:?}, range={:?}, errors={}",
                2..source.len(),
                doc.metadata().error_count
            );
        }
    }
}
