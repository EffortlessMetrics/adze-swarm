//! Generated external-scanner adapter for stack-aware GLR streaming (#857 / #890).
//!
//! Invokes a generated `TSLanguage::external_scanner` at a byte position with the
//! union of valid external symbols supplied by the streaming driver.
//!
//! # Experimental boundary (#931, Option A for 0.10)
//!
//! External scanners are **Experimental** and outside the mandatory Gate A
//! internal-lexer/GLR contract. This adapter makes **no branch-state isolation
//! claim**: one wrapper owns one mutable scanner instance, and the streaming
//! driver evaluates speculative candidates through that shared instance, so
//! sibling GLR branches can observe each other's mutations. The union
//! valid-symbol mask validated here must not be conflated with branch-state
//! correctness.
//!
//! Use [`GeneratedExternalStreamingScanner::snapshot`] and
//! [`GeneratedExternalStreamingScanner::restore`] to save and roll back scanner
//! state around speculative branch evaluation. Wiring one snapshot per GLR
//! branch into the driver is deferred productization work tracked by #931.

#![cfg(all(feature = "glr", feature = "pure-rust", feature = "external_scanners"))]

use adze_glr_core::LexMode;
use adze_glr_core::ts_lexer::NextToken;
use core::ffi::c_void;

use crate::lex::TsLexer;
use crate::pure_parser::TSLanguage;

/// Maximum bytes captured by a single generated-scanner snapshot.
///
/// Bounds snapshot values carried by this wrapper. A serialize callback must
/// respect this buffer capacity; an excessive reported length is an error.
/// This limit does not bound memory owned internally by a scanner callback.
pub const MAX_GENERATED_SCANNER_SNAPSHOT_BYTES: usize = 1024;

/// Structured failure from a single generated external-scanner invocation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StreamingExternalScanError {
    /// The language does not expose an external scanner.
    #[error("generated language is missing external_scanner.scan")]
    MissingScannerFn,
    /// No valid external symbols were supplied for this position.
    #[error("no valid external symbols at byte {pos}")]
    NoValidSymbols {
        /// Byte offset where scanning was attempted.
        pos: usize,
    },
    /// The scanner made no progress at the requested position.
    #[error("external scanner made no progress at byte {pos}")]
    NoProgress {
        /// Byte offset where scanning stalled.
        pos: usize,
    },
    /// The scanner emitted a symbol that is not currently valid.
    #[error("external scanner emitted invalid symbol {symbol} at byte {pos}")]
    InvalidSymbol {
        /// Emitted external symbol index.
        symbol: u16,
        /// Byte offset where scanning started.
        pos: usize,
    },
    /// The scanner produced a zero-width token.
    #[error("external scanner produced a zero-width token at byte {pos}")]
    ZeroWidth {
        /// Byte offset where scanning started.
        pos: usize,
    },
    /// A snapshot exceeds the bounded per-branch scanner state budget.
    #[error("external scanner snapshot is {len} bytes, above the {max}-byte bound")]
    SnapshotTooLarge {
        /// Offered snapshot length in bytes.
        len: usize,
        /// Enforced bound in bytes.
        max: usize,
    },
    /// Scanner state cannot be captured with paired serialize/deserialize hooks.
    #[error("external scanner snapshot requires paired serialize/deserialize hooks")]
    SnapshotUnsupported,
    /// No deserialize hook is available to restore the scanner's state.
    #[error("external scanner has no deserialize hook for snapshot restore")]
    RestoreUnsupported,
}

/// Persistent generated external scanner state for streaming lexing.
pub struct GeneratedExternalStreamingScanner<'a> {
    language: &'a TSLanguage,
    scanner_instance: *mut c_void,
}

impl<'a> GeneratedExternalStreamingScanner<'a> {
    /// Create a scanner wrapper for the given generated language.
    pub fn new(language: &'a TSLanguage) -> Self {
        let scanner_instance = language
            .external_scanner
            .create
            .map(|create| unsafe { create() })
            .unwrap_or(core::ptr::null_mut());

        Self {
            language,
            scanner_instance,
        }
    }

    /// Scan at `pos` when `valid_symbols` is the union mask from active GLR stacks.
    pub fn scan_at(
        &mut self,
        input: &str,
        pos: usize,
        valid_symbols: &[bool],
        _mode: LexMode,
    ) -> Result<Option<NextToken>, StreamingExternalScanError> {
        let scan_fn = self
            .language
            .external_scanner
            .scan
            .ok_or(StreamingExternalScanError::MissingScannerFn)?;

        if pos >= input.len() {
            return Ok(None);
        }

        if !valid_symbols.iter().any(|valid| *valid) {
            return Err(StreamingExternalScanError::NoValidSymbols { pos });
        }

        let ts_valid_symbols = to_tree_sitter_valid_symbols(valid_symbols);
        let source = input.as_bytes();
        let start = pos;
        let token = scan_once(
            self.language,
            scan_fn,
            self.scanner_instance,
            source,
            start,
            &ts_valid_symbols,
        )?;

        let Some((external_index, end)) = token else {
            return Err(StreamingExternalScanError::NoProgress { pos: start });
        };

        if end <= start {
            return Err(StreamingExternalScanError::ZeroWidth { pos: start });
        }

        let driver_index = external_index.saturating_sub(1) as usize;
        if !valid_symbols.get(driver_index).copied().unwrap_or(false) {
            return Err(StreamingExternalScanError::InvalidSymbol {
                symbol: external_index,
                pos: start,
            });
        }

        let kind = map_external_symbol(self.language, external_index);
        Ok(Some(NextToken {
            kind: kind as u32,
            start: start as u32,
            end: end as u32,
        }))
    }

    /// Capture the current scanner state for one GLR branch.
    ///
    /// Requires paired state hooks and rejects an excessive reported length.
    /// Only a language with no create or state hooks uses the empty-state
    /// convention; a missing hook is not proof that allocated state is empty.
    /// Present hooks receive the original instance pointer, including null
    /// when the language does not allocate an instance.
    pub fn snapshot(&self) -> Result<Vec<u8>, StreamingExternalScanError> {
        let hooks = &self.language.external_scanner;
        let serialize = match (hooks.serialize, hooks.deserialize) {
            (Some(serialize), Some(_)) => serialize,
            (None, None) if hooks.create.is_none() && self.scanner_instance.is_null() => {
                return Ok(Vec::new());
            }
            _ => return Err(StreamingExternalScanError::SnapshotUnsupported),
        };
        let mut buffer = [0u8; MAX_GENERATED_SCANNER_SNAPSHOT_BYTES];
        // SAFETY: The language's callback accepts its original instance pointer
        // (possibly null) and must write at most this buffer's capacity. The
        // post-call length check validates the report; it cannot prevent an
        // unsafe callback from violating its write-bound contract.
        let len = unsafe { serialize(self.scanner_instance, buffer.as_mut_ptr()) } as usize;
        if len > buffer.len() {
            return Err(StreamingExternalScanError::SnapshotTooLarge {
                len,
                max: buffer.len(),
            });
        }
        Ok(buffer[..len].to_vec())
    }

    /// Roll the scanner back to a previously captured branch snapshot.
    ///
    /// Always invokes a present deserialize hook, including for empty snapshots
    /// that reset state. Rejects excessive lengths and unavailable restoration;
    /// only the hookless, unallocated empty-state convention is a no-op.
    pub fn restore(&mut self, snapshot: &[u8]) -> Result<(), StreamingExternalScanError> {
        if snapshot.len() > MAX_GENERATED_SCANNER_SNAPSHOT_BYTES {
            return Err(StreamingExternalScanError::SnapshotTooLarge {
                len: snapshot.len(),
                max: MAX_GENERATED_SCANNER_SNAPSHOT_BYTES,
            });
        }
        let Some(deserialize) = self.language.external_scanner.deserialize else {
            if self.language.external_scanner.create.is_none()
                && self.language.external_scanner.serialize.is_none()
                && self.scanner_instance.is_null()
                && snapshot.is_empty()
            {
                return Ok(());
            }
            return Err(StreamingExternalScanError::RestoreUnsupported);
        };
        // SAFETY: The callback accepts its original instance pointer (possibly
        // null). The snapshot outlives the call and the hook contract bounds
        // reads by the passed length, including zero for a reset.
        unsafe {
            deserialize(
                self.scanner_instance,
                snapshot.as_ptr(),
                snapshot.len() as u32,
            );
        }
        Ok(())
    }
}

impl Drop for GeneratedExternalStreamingScanner<'_> {
    fn drop(&mut self) {
        if !self.scanner_instance.is_null()
            && let Some(destroy) = self.language.external_scanner.destroy
        {
            // SAFETY: `scanner_instance` was created by the language `create` hook
            // and is destroyed exactly once here.
            unsafe {
                destroy(self.scanner_instance);
            }
        }
    }
}

/// Build a `Driver::parse_streaming` external-scanner closure for a generated language.
pub fn make_generated_external_streaming_scanner<'a>(
    language: &'a TSLanguage,
) -> GeneratedExternalStreamingScanner<'a> {
    GeneratedExternalStreamingScanner::new(language)
}

fn map_external_symbol(language: &TSLanguage, external_index: u16) -> u16 {
    if language.external_scanner.symbol_map.is_null() {
        return external_index;
    }

    // SAFETY: Tree-sitter external scanners use 1-based indices into `symbol_map`.
    unsafe {
        *language
            .external_scanner
            .symbol_map
            .add(external_index as usize)
    }
}

fn to_tree_sitter_valid_symbols(valid_symbols: &[bool]) -> Vec<bool> {
    let mut ts_valid = vec![false; valid_symbols.len() + 1];
    for (idx, valid) in valid_symbols.iter().enumerate() {
        ts_valid[idx + 1] = *valid;
    }
    ts_valid
}

fn scan_once(
    _language: &TSLanguage,
    scan_fn: unsafe extern "C" fn(*mut c_void, *mut c_void, *const bool) -> bool,
    scanner_instance: *mut c_void,
    source: &[u8],
    pos: usize,
    valid_symbols: &[bool],
) -> Result<Option<(u16, usize)>, StreamingExternalScanError> {
    #[repr(C)]
    struct Backing<'a> {
        input: &'a [u8],
        pos: usize,
        mark: usize,
    }

    unsafe extern "C" fn lookahead(lex: *mut TsLexer) -> u32 {
        unsafe {
            if lex.is_null() || (*lex).data.is_null() {
                return 0;
            }
            let backing = &*((*lex).data as *const Backing);
            if backing.pos < backing.input.len() {
                backing.input[backing.pos] as u32
            } else {
                0
            }
        }
    }

    unsafe extern "C" fn advance(lex: *mut TsLexer, skip: bool) {
        unsafe {
            if lex.is_null() || (*lex).data.is_null() {
                return;
            }
            let backing = &mut *((*lex).data as *mut Backing);
            if backing.pos < backing.input.len() {
                backing.pos += 1;
                if !skip {
                    backing.mark = backing.pos;
                }
            }
        }
    }

    unsafe extern "C" fn mark_end(lex: *mut TsLexer) {
        unsafe {
            if lex.is_null() || (*lex).data.is_null() {
                return;
            }
            let backing = &mut *((*lex).data as *mut Backing);
            backing.mark = backing.pos;
        }
    }

    let mut backing = Backing {
        input: &source[pos..],
        pos: 0,
        mark: 0,
    };
    let mut ts_lexer = TsLexer {
        lookahead,
        advance,
        mark_end,
        result_symbol: u16::MAX,
        data: &mut backing as *mut _ as *mut c_void,
    };

    // SAFETY: `scan_fn` is the generated external scanner. `valid_symbols` is a
    // live slice for the duration of the call. `scanner_instance` may be null when
    // the grammar does not allocate scanner state.
    let ok = unsafe {
        scan_fn(
            scanner_instance,
            &mut ts_lexer as *mut _ as *mut c_void,
            valid_symbols.as_ptr(),
        )
    };

    if !ok || ts_lexer.result_symbol == u16::MAX || ts_lexer.result_symbol == 0 {
        return Ok(None);
    }

    let end = if backing.mark > 0 {
        pos + backing.mark
    } else {
        pos + backing.pos
    };

    Ok(Some((ts_lexer.result_symbol, end)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pure_parser::{ExternalScanner, SyncPtr, TSParseAction, TSRule};
    use core::ptr;

    unsafe extern "C" fn stub_scan_fn(
        _scanner: *mut c_void,
        lexer: *mut c_void,
        valid_symbols: *const bool,
    ) -> bool {
        if lexer.is_null() || valid_symbols.is_null() {
            return false;
        }
        let lexer = lexer as *mut TsLexer;
        let valid = unsafe { *valid_symbols.add(1) };
        if !valid {
            return false;
        }
        let first = unsafe { ((*lexer).lookahead)(lexer) };
        if first == b'x' as u32 {
            unsafe {
                ((*lexer).advance)(lexer, false);
                (*lexer).result_symbol = 1;
                ((*lexer).mark_end)(lexer);
            }
            return true;
        }
        false
    }

    fn stub_language(
        scan: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *const bool) -> bool>,
    ) -> TSLanguage {
        static SYMBOL_MAP: [u16; 2] = [0, 42];
        TSLanguage {
            version: 15,
            symbol_count: 8,
            alias_count: 0,
            token_count: 4,
            external_token_count: 1,
            state_count: 1,
            large_state_count: 0,
            production_id_count: 0,
            field_count: 0,
            max_alias_sequence_length: 0,
            production_id_map: ptr::null(),
            parse_table: ptr::null(),
            small_parse_table: ptr::null(),
            small_parse_table_map: ptr::null(),
            parse_actions: ptr::null::<TSParseAction>(),
            symbol_names: ptr::null(),
            field_names: ptr::null(),
            field_map_slices: ptr::null(),
            field_map_entries: ptr::null(),
            symbol_metadata: ptr::null(),
            public_symbol_map: ptr::null(),
            alias_map: ptr::null(),
            alias_sequences: ptr::null(),
            lex_modes: ptr::null(),
            lex_fn: None,
            keyword_lex_fn: None,
            keyword_capture_token: 0,
            external_scanner: ExternalScanner {
                states: ptr::null(),
                symbol_map: SYMBOL_MAP.as_ptr(),
                create: None,
                destroy: None,
                scan,
                serialize: None,
                deserialize: None,
            },
            primary_state_ids: ptr::null(),
            production_lhs_index: ptr::null(),
            production_count: 0,
            eof_symbol: 0,
            rules: ptr::null::<TSRule>(),
            rule_count: 0,
        }
    }

    #[test]
    fn external_scanner_adapter_honors_valid_symbol_mask() {
        let language = stub_language(Some(stub_scan_fn));
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);
        let valid = [true];

        let token = scanner
            .scan_at(
                "x",
                0,
                &valid,
                LexMode {
                    lex_state: 0,
                    external_lex_state: 0b001,
                },
            )
            .expect("scan should succeed")
            .expect("token expected");

        assert_eq!(token.kind, 42);
        assert_eq!(token.start, 0);
        assert_eq!(token.end, 1);
    }

    #[test]
    fn external_scanner_adapter_rejects_invalid_symbol_emission() {
        let language = stub_language(Some(stub_scan_fn));
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);
        let valid = [false];

        let err = scanner
            .scan_at(
                "x",
                0,
                &valid,
                LexMode {
                    lex_state: 0,
                    external_lex_state: 0,
                },
            )
            .expect_err("invalid mask should fail");

        assert_eq!(
            err,
            StreamingExternalScanError::NoValidSymbols { pos: 0 },
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn test_stateful_scan_mutations_persist_across_wrapper_calls() {
        // Characterization of the single-shared-instance seam (#931): one
        // wrapper owns one scanner instance, so a stateful scan is visible to
        // every later evaluation. Competing GLR branches are NOT isolated.
        let language = stateful_language();
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);
        let valid = [true];
        let mode = LexMode {
            lex_state: 0,
            external_lex_state: 0b001,
        };

        scanner
            .scan_at("x", 0, &valid, mode)
            .expect("first scan should succeed")
            .expect("token expected");
        scanner
            .scan_at("x", 0, &valid, mode)
            .expect("second scan should succeed")
            .expect("token expected");

        assert_eq!(
            counter_of(&scanner),
            2,
            "shared instance must observe both evaluations (no branch isolation)"
        );
    }

    #[test]
    fn test_snapshot_restore_round_trip_restores_counter() {
        // Manual wrapper round trip; this does not execute driver forks or
        // prove branch-local wiring. The counter has an independent oracle.
        let language = stateful_language();
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);
        let valid = [true];
        let mode = LexMode {
            lex_state: 0,
            external_lex_state: 0b001,
        };

        let clean = scanner.snapshot().unwrap();
        assert_eq!(clean, vec![0, 0, 0, 0]);
        scanner
            .scan_at("x", 0, &valid, mode)
            .expect("branch scan should succeed")
            .expect("token expected");
        assert_eq!(counter_of(&scanner), 1);
        scanner.restore(&clean).expect("restore should succeed");
        assert_eq!(
            counter_of(&scanner),
            0,
            "restore must roll back speculative branch state"
        );
    }

    #[test]
    fn test_restore_oversized_snapshot_returns_error() {
        // Snapshots are bounded (#931 acceptance): an over-limit snapshot is a
        // structured failure, never a silent truncation.
        let language = stateful_language();
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);
        let oversized = vec![0u8; MAX_GENERATED_SCANNER_SNAPSHOT_BYTES + 1];

        let err = scanner
            .restore(&oversized)
            .expect_err("oversized snapshot must fail");
        assert_eq!(
            err,
            StreamingExternalScanError::SnapshotTooLarge {
                len: oversized.len(),
                max: MAX_GENERATED_SCANNER_SNAPSHOT_BYTES,
            }
        );
    }

    #[test]
    fn test_restore_nonempty_without_deserialize_returns_error() {
        // A language without a deserialize hook cannot restore non-empty
        // state: report it instead of silently ignoring the snapshot.
        let language = stub_language(Some(stub_scan_fn));
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);

        let err = scanner
            .restore(&[1, 2, 3])
            .expect_err("hookless restore must fail");
        assert_eq!(err, StreamingExternalScanError::RestoreUnsupported);
    }

    #[test]
    fn test_hookless_scanner_without_instance_has_empty_state() {
        // This fixture has no create hook and no state hooks. Missing hooks
        // on an allocated instance are tested separately and must fail.
        let language = stub_language(Some(stub_scan_fn));
        let scanner = GeneratedExternalStreamingScanner::new(&language);
        assert_eq!(scanner.snapshot(), Ok(Vec::new()));
    }

    #[test]
    fn test_restore_empty_missing_hooks_does_not_report_rollback() {
        let mut language = stateful_language();
        language.external_scanner.serialize = None;
        language.external_scanner.deserialize = None;
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);
        let token = scanner
            .scan_at(
                "x",
                0,
                &[true],
                LexMode {
                    lex_state: 0,
                    external_lex_state: 1,
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!((token.kind, token.start, token.end), (42, 0, 1));
        assert_eq!(counter_of(&scanner), 1);
        assert_eq!(
            scanner.restore(&[]),
            Err(StreamingExternalScanError::RestoreUnsupported)
        );
        assert_eq!(counter_of(&scanner), 1);
    }

    #[test]
    fn test_snapshot_reported_oversize_returns_error_without_truncation() {
        let mut language = stateful_language();
        language.external_scanner.serialize = Some(oversized_report_serialize);
        let scanner = GeneratedExternalStreamingScanner::new(&language);
        assert_eq!(
            scanner.snapshot(),
            Err(StreamingExternalScanError::SnapshotTooLarge {
                len: MAX_GENERATED_SCANNER_SNAPSHOT_BYTES + 1,
                max: MAX_GENERATED_SCANNER_SNAPSHOT_BYTES,
            })
        );
        assert_eq!(counter_of(&scanner), 0);
    }

    #[test]
    fn test_snapshot_allocated_state_requires_paired_hooks() {
        for (missing_serialize, missing_deserialize) in [(true, true), (true, false), (false, true)]
        {
            let mut language = stateful_language();
            if missing_serialize {
                language.external_scanner.serialize = None;
            }
            if missing_deserialize {
                language.external_scanner.deserialize = None;
            }
            let scanner = GeneratedExternalStreamingScanner::new(&language);
            assert_eq!(
                scanner.snapshot(),
                Err(StreamingExternalScanError::SnapshotUnsupported)
            );
            assert_eq!(counter_of(&scanner), 0);
        }
    }

    #[test]
    fn test_restore_empty_invokes_reset_hook_with_null_buffer() {
        let mut language = stateful_language();
        language.external_scanner.deserialize = Some(resettable_deserialize);
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);
        scanner.restore(&7u32.to_le_bytes()).unwrap();
        assert_eq!(counter_of(&scanner), 7);
        scanner.restore(&[]).unwrap();
        assert_eq!(counter_of(&scanner), 0);
        assert_eq!(scanner.snapshot(), Ok(vec![0, 0, 0, 0]));
    }

    #[test]
    fn test_snapshot_full_capacity_round_trip_preserves_bytes() {
        let mut language = stateful_language();
        language.external_scanner.serialize = Some(full_capacity_serialize);
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);
        let snapshot = scanner.snapshot().unwrap();
        let expected: Vec<u8> = (0..MAX_GENERATED_SCANNER_SNAPSHOT_BYTES)
            .map(|index| index as u8)
            .collect();
        assert_eq!(snapshot, expected);
        scanner.restore(&snapshot).unwrap();
        assert_eq!(counter_of(&scanner), u32::from_le_bytes([0, 1, 2, 3]));
    }

    #[test]
    fn test_hookless_null_creator_does_not_claim_stateless_rollback() {
        let mut language = stub_language(Some(stub_scan_fn));
        language.external_scanner.create = Some(null_create);
        let mut scanner = GeneratedExternalStreamingScanner::new(&language);
        assert_eq!(
            scanner.snapshot(),
            Err(StreamingExternalScanError::SnapshotUnsupported)
        );
        assert_eq!(
            scanner.restore(&[]),
            Err(StreamingExternalScanError::RestoreUnsupported)
        );
    }

    unsafe extern "C" fn oversized_report_serialize(scanner: *mut c_void, buffer: *mut u8) -> u32 {
        // The fixture writes only four valid bytes, then reports an impossible
        // length. It does not perform an out-of-bounds write to test the guard.
        let _ = unsafe { stateful_serialize(scanner, buffer) };
        (MAX_GENERATED_SCANNER_SNAPSHOT_BYTES + 1) as u32
    }

    unsafe extern "C" fn full_capacity_serialize(_scanner: *mut c_void, buffer: *mut u8) -> u32 {
        for index in 0..MAX_GENERATED_SCANNER_SNAPSHOT_BYTES {
            unsafe { *buffer.add(index) = index as u8 };
        }
        MAX_GENERATED_SCANNER_SNAPSHOT_BYTES as u32
    }

    unsafe extern "C" fn resettable_deserialize(
        scanner: *mut c_void,
        buffer: *const u8,
        length: u32,
    ) {
        if length == 0 && buffer.is_null() && !scanner.is_null() {
            unsafe { *(scanner as *mut u32) = 0 };
        } else {
            unsafe { stateful_deserialize(scanner, buffer, length) };
        }
    }

    unsafe extern "C" fn null_create() -> *mut c_void {
        core::ptr::null_mut()
    }

    unsafe extern "C" fn stateful_create() -> *mut c_void {
        Box::into_raw(Box::new(0u32)) as *mut c_void
    }

    unsafe extern "C" fn stateful_destroy(scanner: *mut c_void) {
        if !scanner.is_null() {
            unsafe {
                let _ = Box::from_raw(scanner as *mut u32);
            }
        }
    }

    unsafe extern "C" fn stateful_scan_fn(
        scanner: *mut c_void,
        lexer: *mut c_void,
        valid_symbols: *const bool,
    ) -> bool {
        if scanner.is_null() {
            return false;
        }
        unsafe {
            let counter = &mut *(scanner as *mut u32);
            *counter = counter.wrapping_add(1);
        }
        unsafe { stub_scan_fn(scanner, lexer, valid_symbols) }
    }

    unsafe extern "C" fn stateful_serialize(scanner: *mut c_void, buffer: *mut u8) -> u32 {
        if scanner.is_null() || buffer.is_null() {
            return 0;
        }
        unsafe {
            let counter = *(scanner as *const u32);
            let bytes = counter.to_le_bytes();
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
            bytes.len() as u32
        }
    }

    unsafe extern "C" fn stateful_deserialize(
        scanner: *mut c_void,
        buffer: *const u8,
        length: u32,
    ) {
        if scanner.is_null() || buffer.is_null() || length < 4 {
            return;
        }
        unsafe {
            let mut bytes = [0u8; 4];
            core::ptr::copy_nonoverlapping(buffer, bytes.as_mut_ptr(), 4);
            *(scanner as *mut u32) = u32::from_le_bytes(bytes);
        }
    }

    fn stateful_language() -> TSLanguage {
        let mut language = stub_language(Some(stateful_scan_fn));
        language.external_scanner.create = Some(stateful_create);
        language.external_scanner.destroy = Some(stateful_destroy);
        language.external_scanner.serialize = Some(stateful_serialize);
        language.external_scanner.deserialize = Some(stateful_deserialize);
        language
    }

    fn counter_of(scanner: &GeneratedExternalStreamingScanner) -> u32 {
        assert!(!scanner.scanner_instance.is_null());
        unsafe { *(scanner.scanner_instance as *const u32) }
    }
}
