//! #983: the symbol-normalization explanation must describe the code as
//! it is. This contract fails when the page drifts back into claiming
//! behavior the implementation does not have.

use std::path::Path;

fn normalization_doc() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("explanations")
        .join("symbol-normalization.md");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

#[test]
fn doc_does_not_claim_fallible_normalize_or_mutating_compute() {
    let doc = normalization_doc();
    assert!(
        !doc.contains("normalize()?"),
        "normalize() returns Vec<Rule>, not a Result"
    );
    assert!(
        !doc.contains("compute(&mut grammar)"),
        "FirstFollowSets::compute takes &Grammar and clones internally"
    );
}

#[test]
fn doc_does_not_claim_unique_production_ids_or_aux_rule_names() {
    let doc = normalization_doc();
    assert!(
        !doc.contains("unique `ProductionId`"),
        "auxiliary rules currently share ProductionId(0)"
    );
    assert!(
        !doc.contains("get generated rule names"),
        "normalization does not add _aux entries to rule_names"
    );
    assert!(
        !doc.contains("Generated parser behavior is identical"),
        "no identical-behavior guarantee is proved"
    );
}

#[test]
fn doc_states_exact_signatures_and_identity_model() {
    let doc = normalization_doc();
    for needle in [
        "normalize(&mut self) -> Vec<Rule>",
        "compute(&Grammar)",
        "compute_normalized(&mut Grammar)",
        "ProductionId(0)",
        "RuleId",
    ] {
        assert!(doc.contains(needle), "doc must state {needle}");
    }
}

#[test]
fn doc_links_tracked_defects_and_repairs() {
    let doc = normalization_doc();
    for issue in ["978", "980", "981", "982"] {
        assert!(
            doc.contains(&format!("issues/{issue}")),
            "doc must link issue #{issue}"
        );
    }
}
