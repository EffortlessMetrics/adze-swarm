//! Downstream contract proof for the open `Extract` trait (#865).
//!
//! Integration tests compile as their own crate that depends on `adze`, so
//! everything this file does is something an ordinary downstream user can do.
//! That makes it the compile-pass half of the contract recorded in
//! `docs/adr/ADZE-ADR-0008-extract-is-an-open-trait.md`: `Extract` is
//! intentionally open, and `#[adze::grammar]` relies on that because it
//! expands in the caller's crate.
//!
//! If `Extract` is ever genuinely sealed, this file stops compiling. That is
//! the point — the ADR has to be revisited before the seal lands.

use adze::Extract;
use adze::pure_parser::ParsedNode;

/// A type declared outside `adze`, exactly like a downstream user's type.
struct DownstreamLeaf;

/// A hand-written `Extract` implementation from outside the crate.
impl Extract<u32> for DownstreamLeaf {
    type LeafFn = ();

    fn extract(
        node: Option<&ParsedNode>,
        source: &[u8],
        _last_idx: usize,
        _leaf_fn: Option<&Self::LeafFn>,
    ) -> u32 {
        node.and_then(|node| source.get(node.start_byte()..node.end_byte()))
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .and_then(|text| text.parse().ok())
            .unwrap_or_default()
    }
}

#[test]
fn downstream_crate_can_implement_extract() {
    // Compiling at all is the assertion; calling it keeps the impl live.
    assert_eq!(DownstreamLeaf::extract(None, b"", 0, None), 0);
}

#[test]
fn extract_bound_needs_no_marker_supertrait() {
    // `Extract` alone is a sufficient bound: satisfying it requires no
    // separate marker implementation from the downstream crate.
    fn accepts_any_extractor<T: Extract<u32>>() {}
    accepts_any_extractor::<DownstreamLeaf>();
}

#[test]
fn added_trait_items_default_for_downstream_impls() {
    // The evolution rule in ADZE-ADR-0008: items added to `Extract` carry
    // defaults, so a downstream impl that names none of them still compiles.
    const { assert!(!<DownstreamLeaf as Extract<u32>>::HAS_CONFLICTS) };
    assert_eq!(<DownstreamLeaf as Extract<u32>>::GRAMMAR_NAME, "unknown");
    assert_eq!(<DownstreamLeaf as Extract<u32>>::GRAMMAR_JSON, "{}");
}
