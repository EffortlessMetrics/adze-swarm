// Lexicographic symbol comparison for Tree-sitter conflict resolution
//! Lexicographic symbol comparison as a tie-breaker for conflict resolution.

// This implements the final tie-breaker when all other comparisons are equal

use crate::CompareResult;
use adze_ir::SymbolId;

/// Compare two parse trees by their root symbols lexicographically
/// This is Tree-sitter's final tie-breaker for conflict resolution
pub fn compare_symbols(left_symbol: SymbolId, right_symbol: SymbolId) -> CompareResult {
    match left_symbol.0.cmp(&right_symbol.0) {
        std::cmp::Ordering::Less => CompareResult::TakeLeft,
        std::cmp::Ordering::Greater => CompareResult::TakeRight,
        std::cmp::Ordering::Equal => CompareResult::Tie,
    }
}

/// Extended comparison that includes symbol comparison as final tie-breaker
pub fn compare_versions_with_symbols(
    left_version: &crate::VersionInfo,
    right_version: &crate::VersionInfo,
    left_symbol: SymbolId,
    right_symbol: SymbolId,
) -> CompareResult {
    // First, use the standard version comparison
    let version_result = crate::compare_versions(left_version, right_version);

    // If versions are tied, use symbol comparison
    match version_result {
        CompareResult::Tie => compare_symbols(left_symbol, right_symbol),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_comparison() {
        // Lower symbol ID wins
        assert_eq!(
            compare_symbols(SymbolId(10), SymbolId(20)),
            CompareResult::TakeLeft
        );

        assert_eq!(
            compare_symbols(SymbolId(30), SymbolId(15)),
            CompareResult::TakeRight
        );

        assert_eq!(
            compare_symbols(SymbolId(42), SymbolId(42)),
            CompareResult::Tie
        );
    }

    #[test]
    fn test_full_comparison_with_symbols() {
        let v1 = crate::VersionInfo::new();
        let v2 = crate::VersionInfo::new();

        // When versions are equal, symbols are the tie-breaker
        assert_eq!(
            compare_versions_with_symbols(&v1, &v2, SymbolId(1), SymbolId(2)),
            CompareResult::TakeLeft
        );

        // When versions differ, symbols are ignored
        let mut v3 = crate::VersionInfo::new();
        v3.add_dynamic_prec(5);

        assert_eq!(
            compare_versions_with_symbols(&v3, &v1, SymbolId(100), SymbolId(1)),
            CompareResult::TakeLeft // v3 wins due to higher dynamic precedence
        );
    }

    #[test]
    fn test_compare_symbols_zero_ids() {
        // SymbolId(0) is a valid identifier (often EOF / start symbol); make
        // sure the boundary value still routes through the comparison.
        assert_eq!(
            compare_symbols(SymbolId(0), SymbolId(1)),
            CompareResult::TakeLeft
        );
        assert_eq!(
            compare_symbols(SymbolId(0), SymbolId(0)),
            CompareResult::Tie
        );
        assert_eq!(
            compare_symbols(SymbolId(1), SymbolId(0)),
            CompareResult::TakeRight
        );
    }

    #[test]
    fn test_compare_symbols_max_boundary() {
        // Pin behaviour at the SymbolId (u16) upper boundary to defend
        // against accidental signed/unsigned mix-ups in the comparator.
        let max = SymbolId(u16::MAX);
        let max_minus_one = SymbolId(u16::MAX - 1);
        assert_eq!(compare_symbols(max_minus_one, max), CompareResult::TakeLeft);
        assert_eq!(
            compare_symbols(max, max_minus_one),
            CompareResult::TakeRight
        );
        assert_eq!(compare_symbols(max, max), CompareResult::Tie);
    }

    #[test]
    fn test_full_comparison_tie_take_right() {
        // The previously-untested half of the symbol tie-breaker arm:
        // when version comparison ties and the right symbol is the smaller
        // one, the tie-breaker must select the right side.
        let v1 = crate::VersionInfo::new();
        let v2 = crate::VersionInfo::new();

        assert_eq!(
            compare_versions_with_symbols(&v1, &v2, SymbolId(7), SymbolId(3)),
            CompareResult::TakeRight
        );
    }

    #[test]
    fn test_full_comparison_tie_returns_tie_when_symbols_equal() {
        // Versions tie AND symbols tie => the final result must still be Tie,
        // ensuring `compare_symbols` propagates Equal through the wrapper.
        let v1 = crate::VersionInfo::new();
        let v2 = crate::VersionInfo::new();

        assert_eq!(
            compare_versions_with_symbols(&v1, &v2, SymbolId(9), SymbolId(9)),
            CompareResult::Tie
        );
    }

    #[test]
    fn test_full_comparison_version_take_right_ignores_symbols() {
        // Force the `other => other` arm to surface TakeRight by giving the
        // right side strictly higher dynamic precedence. Symbol order is
        // deliberately picked so a fall-through to the tie-breaker would
        // flip the answer to TakeLeft.
        let v_low = crate::VersionInfo::new();
        let mut v_high = crate::VersionInfo::new();
        v_high.add_dynamic_prec(7);

        assert_eq!(
            compare_versions_with_symbols(&v_low, &v_high, SymbolId(1), SymbolId(99)),
            CompareResult::TakeRight
        );
    }

    #[test]
    fn test_full_comparison_version_prefer_left_ignores_symbols() {
        // Small cost difference => PreferLeft from compare_versions. Confirm
        // the wrapper passes it through and ignores the symbols.
        let mut v_cheap = crate::VersionInfo::new();
        let mut v_costly = crate::VersionInfo::new();
        v_cheap.add_error_cost(100, 1);
        v_costly.add_error_cost(200, 1);

        assert_eq!(
            compare_versions_with_symbols(&v_cheap, &v_costly, SymbolId(999), SymbolId(1)),
            CompareResult::PreferLeft
        );
    }

    #[test]
    fn test_full_comparison_version_prefer_right_ignores_symbols() {
        // Mirror image of PreferLeft: ensure PreferRight is also forwarded
        // unchanged and does not get short-circuited into the tie-breaker.
        let mut v_costly = crate::VersionInfo::new();
        let mut v_cheap = crate::VersionInfo::new();
        v_costly.add_error_cost(200, 1);
        v_cheap.add_error_cost(100, 1);

        assert_eq!(
            compare_versions_with_symbols(&v_costly, &v_cheap, SymbolId(1), SymbolId(999)),
            CompareResult::PreferRight
        );
    }

    #[test]
    fn test_full_comparison_error_take_right_ignores_symbols() {
        // Error-vs-non-error path returns TakeRight directly. Pick symbols
        // that would otherwise produce TakeLeft via the tie-breaker.
        let mut v_err = crate::VersionInfo::new();
        v_err.enter_error();
        let v_ok = crate::VersionInfo::new();

        assert_eq!(
            compare_versions_with_symbols(&v_err, &v_ok, SymbolId(1), SymbolId(2)),
            CompareResult::TakeRight
        );
    }
}
