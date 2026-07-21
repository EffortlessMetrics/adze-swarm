//! Lexical commitment contract for stack-aware GLR streaming lexing (#928).
//!
//! Policy: **global lexical commitment**. At each byte position the driver gathers
//! candidates from every distinct active lex mode, selects one global winner, and
//! applies that token to all active stacks. Stacks that cannot shift the committed
//! token are pruned by the ordinary GLR action table (they produce no successor
//! stacks). This is intentional: lexical branching is out of scope for 0.10.

use crate::driver::GlrError;
use crate::ts_lexer::NextToken;
use crate::{ParseTable, StateId, SymbolId};

/// Stable policy identifier recorded by tests and diagnostics.
pub const GLOBAL_LEXICAL_COMMITMENT_POLICY: &str = "global";

/// Maximum token candidates considered at one position before failing closed.
pub const MAX_LEX_CANDIDATES_PER_POSITION: usize = 64;

/// Whether a candidate token originated from the internal or external lexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateOrigin {
    /// Generated/internal lexer (`lex_fn`).
    Internal,
    /// External scanner callback.
    External,
}

/// One lexical candidate at the current position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenCandidate {
    pub token: NextToken,
    pub origin: CandidateOrigin,
}

/// Deterministic reason the global winner was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexicalSelectionReason {
    SoleCandidate,
    LongerMatch,
    ActionableOverNonActionable,
    PreferredInternalOrigin,
    LowerSymbolId,
}

/// Result of global lexical commitment at one position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LexicalSelection {
    pub token: NextToken,
    pub reason: LexicalSelectionReason,
}

/// Indices of active stacks whose top state accepts an action on `kind`.
#[must_use]
pub fn compatible_stack_indices(
    tables: &ParseTable,
    stack_tops: &[StateId],
    kind: u32,
) -> Vec<usize> {
    let sym = SymbolId(kind as u16);
    stack_tops
        .iter()
        .enumerate()
        .filter_map(|(idx, top)| {
            if tables.actions(*top, sym).is_empty() {
                None
            } else {
                Some(idx)
            }
        })
        .collect()
}

/// Whether any active stack can act on `kind`.
#[must_use]
pub fn has_action_for_any_stack(tables: &ParseTable, stack_tops: &[StateId], kind: u32) -> bool {
    !compatible_stack_indices(tables, stack_tops, kind).is_empty()
}

/// Select the global lexical commitment for divergent stack modes.
pub fn select_global_lexical_candidate(
    tables: &ParseTable,
    candidates: &[TokenCandidate],
    stack_tops: &[StateId],
) -> Result<LexicalSelection, GlrError> {
    if candidates.is_empty() {
        return Err(GlrError::Parse(
            "no valid token candidate at lexical commitment point".to_string(),
        ));
    }
    if candidates.len() > MAX_LEX_CANDIDATES_PER_POSITION {
        return Err(GlrError::Parse(format!(
            "lexical candidate limit ({}) exceeded at commitment point",
            MAX_LEX_CANDIDATES_PER_POSITION
        )));
    }

    let mut best: Option<(TokenCandidate, LexicalSelectionReason)> = None;

    for candidate in candidates {
        let token = candidate.token;

        let Some((best_candidate, _best_reason)) = best else {
            best = Some((*candidate, LexicalSelectionReason::SoleCandidate));
            continue;
        };

        let best_len = (best_candidate.token.end - best_candidate.token.start) as i64;
        let cand_len = (token.end - token.start) as i64;

        if cand_len > best_len {
            best = Some((*candidate, LexicalSelectionReason::LongerMatch));
            continue;
        }
        if cand_len < best_len {
            continue;
        }

        let cand_actionable = has_action_for_any_stack(tables, stack_tops, token.kind);
        let best_actionable =
            has_action_for_any_stack(tables, stack_tops, best_candidate.token.kind);

        if cand_actionable && !best_actionable {
            best = Some((
                *candidate,
                LexicalSelectionReason::ActionableOverNonActionable,
            ));
            continue;
        }
        if !cand_actionable && best_actionable {
            continue;
        }

        if candidate.origin == CandidateOrigin::Internal
            && best_candidate.origin == CandidateOrigin::External
        {
            best = Some((*candidate, LexicalSelectionReason::PreferredInternalOrigin));
            continue;
        }
        if candidate.origin == CandidateOrigin::External
            && best_candidate.origin == CandidateOrigin::Internal
        {
            continue;
        }

        if token.kind < best_candidate.token.kind {
            best = Some((*candidate, LexicalSelectionReason::LowerSymbolId));
        }
    }

    best.map(|(candidate, reason)| LexicalSelection {
        token: candidate.token,
        reason,
    })
    .ok_or_else(|| {
        GlrError::Parse("no valid token candidate after lexical commitment filtering".to_string())
    })
}
