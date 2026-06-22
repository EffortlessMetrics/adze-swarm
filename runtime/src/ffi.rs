//! FFI types and functions for bridging between C and Rust interfaces.
#![cfg_attr(feature = "strict_docs", allow(missing_docs))]

use crate::external_scanner_ffi::TSLexer;

// Re-export types from external_scanner_ffi
pub use crate::external_scanner_ffi::TSExternalScannerData;

// Type alias for TSSymbol
pub type TSSymbol = u16;

/// Tree-sitter symbol metadata
#[repr(C)]
pub struct TSSymbolMetadata {
    pub visible: bool,
    pub named: bool,
    pub supertype: bool,
}

/// Tree-sitter parse action types
#[repr(C)]
pub enum TSParseActionType {
    Shift = 0,
    Reduce = 1,
    Accept = 2,
    Error = 3,
}

/// Tree-sitter parse action entry
#[repr(C)]
pub struct TSParseActionEntry {
    pub type_: TSParseActionType,
    pub state: u16,
    pub symbol: u16,
    pub child_count: u8,
    pub production_id: u8,
}

/// Runtime state for the lexer adapter
pub struct LexerAdapterState {
    /// Input buffer
    pub input: *const u8,
    /// Current position in the input
    pub position: usize,
    /// Length of the input
    pub length: usize,
    /// End position of the current token
    pub token_end: usize,
    /// Current lookahead character
    pub lookahead: u32,
}

/// Create a lexer adapter for use in scan functions
///
/// This function creates a TSLexer struct that the external scanner can use
/// to read input and mark token boundaries.
///
/// # Safety
///
/// - `input` must point to a valid byte buffer of at least `length` bytes.
/// - The buffer must remain valid for the lifetime of the returned pointers.
/// - `position` must be ≤ `length`.
pub unsafe fn create_lexer_adapter(
    input: *const u8,
    position: usize,
    length: usize,
) -> (*mut TSLexer, *mut LexerAdapterState) {
    // Create the adapter state
    let mut initial_lookahead = 0u32;
    if position < length {
        // SAFETY: Caller guarantees `input` points to a valid buffer of at least
        // `length` bytes, and the branch guard ensures `position < length`.
        unsafe {
            initial_lookahead = *input.add(position) as u32;
        }
    }

    let state = Box::new(LexerAdapterState {
        input,
        position,
        length,
        token_end: position,
        lookahead: initial_lookahead,
    });
    let state_ptr = Box::into_raw(state);

    // Create the TSLexer struct with function pointers
    let lexer = Box::new(TSLexer {
        lookahead: ts_lexer_lookahead,
        advance: ts_lexer_advance,
        mark_end: ts_lexer_mark_end,
        get_column: ts_lexer_get_column,
        is_at_included_range_start: ts_lexer_is_at_included_range_start,
        eof: ts_lexer_eof,
        context: state_ptr.cast(), // Store the lexer state as context
        result_symbol: 0,
    });
    let lexer_ptr = Box::into_raw(lexer);

    (lexer_ptr, state_ptr)
}

/// Clean up the lexer adapter
///
/// # Safety
///
/// - `lexer` must be null or the `*mut TSLexer` returned by `create_lexer_adapter`.
/// - `state` must be null or the `*mut LexerAdapterState` returned by
///   `create_lexer_adapter` (the second element of the returned tuple).
/// - If both `lexer` and `state` are non-null and `state` equals the pointer
///   stored in `lexer.context` (the normal case when the caller passes the
///   original pair), the state is freed exactly once via `state_ptr` and the
///   `state != state_ptr` guard prevents a double-free.
/// - Each pointer must not have been freed previously (no double-free).
/// - Passing a pointer that was NOT returned by `create_lexer_adapter` is UB.
pub unsafe fn destroy_lexer_adapter(lexer: *mut TSLexer, state: *mut LexerAdapterState) {
    let state_ptr = if lexer.is_null() {
        state
    } else {
        // SAFETY: `lexer` is non-null (branch guard) and was created by
        // `create_lexer_adapter` via `Box::into_raw`, so dereferencing is valid.
        unsafe { (*lexer).context as *mut LexerAdapterState }
    };

    if !lexer.is_null() {
        // SAFETY: `lexer` was allocated by `Box::into_raw` in `create_lexer_adapter`
        // and is non-null (branch guard). We consume it exactly once here.
        let _ = unsafe { Box::from_raw(lexer) };
    }
    if !state_ptr.is_null() {
        // SAFETY: `state_ptr` was obtained from `lexer.context` which was set to
        // a `Box::into_raw(state)` pointer in `create_lexer_adapter`. Non-null guard above.
        let _ = unsafe { Box::from_raw(state_ptr) };
    }
    if !state.is_null() && state != state_ptr {
        // SAFETY: `state` is non-null and differs from `state_ptr` by address
        // comparison, so it points to a separate allocation not freed above.
        // The `state != state_ptr` guard prevents the double-free case documented
        // in the function-level safety contract: if the caller passes the same
        // pointer value as `state` that `lexer.context` holds, `state_ptr == state`
        // and this branch is skipped, freeing the state exactly once via `state_ptr`.
        let _ = unsafe { Box::from_raw(state) };
    }
}

#[inline]
unsafe fn lexer_state(lexer: *mut TSLexer) -> *mut LexerAdapterState {
    // SAFETY: Caller guarantees `lexer` is a valid, non-null pointer created by
    // `create_lexer_adapter`. The `context` field holds a `LexerAdapterState` pointer.
    unsafe { (*lexer).context as *mut LexerAdapterState }
}

#[inline]
unsafe fn lexer_state_const(lexer: *const TSLexer) -> *const LexerAdapterState {
    // SAFETY: Caller guarantees `lexer` is a valid, non-null pointer created by
    // `create_lexer_adapter`. The `context` field holds a `LexerAdapterState` pointer.
    unsafe { (*lexer).context as *const LexerAdapterState }
}

// Callback functions for TSLexer

extern "C" fn ts_lexer_lookahead(lexer: *mut TSLexer) -> u32 {
    // SAFETY: `lexer` is provided by Tree-sitter runtime and points to a TSLexer
    // created by `create_lexer_adapter`. `lexer_state` returns the context pointer
    // which is validated for null below. The shared reference `&*state_ptr` is safe
    // because no mutable alias exists during this callback's execution.
    unsafe {
        let state_ptr = lexer_state(lexer);
        if state_ptr.is_null() {
            return 0;
        }
        let state = &*state_ptr;
        state.lookahead
    }
}

extern "C" fn ts_lexer_advance(lexer: *mut TSLexer, _skip: bool) {
    // SAFETY: `lexer` is provided by Tree-sitter runtime and points to a TSLexer
    // created by `create_lexer_adapter`. `lexer_state` is null-checked below.
    // The mutable reference `&mut *state_ptr` is safe because Tree-sitter
    // guarantees single-threaded callback invocation (no concurrent access).
    // Pointer arithmetic on `state.input.add(position)` is valid because
    // `position < length` is checked, and the input buffer has `length` bytes.
    unsafe {
        let state_ptr = lexer_state(lexer);
        if state_ptr.is_null() {
            return;
        }
        let state = &mut *state_ptr;

        if state.position < state.length {
            state.position += 1;
            // Update the lookahead character in state
            if state.position < state.length {
                let byte = *state.input.add(state.position);
                state.lookahead = byte as u32;
            } else {
                state.lookahead = 0; // EOF
            }
        }
    }
}

extern "C" fn ts_lexer_mark_end(lexer: *mut TSLexer) {
    // SAFETY: `lexer` points to a TSLexer from `create_lexer_adapter`.
    // `lexer_state` is null-checked below. Mutable reference is safe because
    // Tree-sitter guarantees single-threaded callback invocation.
    unsafe {
        let state_ptr = lexer_state(lexer);
        if state_ptr.is_null() {
            return;
        }
        let state = &mut *state_ptr;
        state.token_end = state.position;
    }
}

extern "C" fn ts_lexer_get_column(lexer: *mut TSLexer) -> u32 {
    // SAFETY: `lexer` points to a TSLexer from `create_lexer_adapter`.
    // `lexer_state` is null-checked below. Shared reference is safe (no mutation).
    // Pointer arithmetic `state.input.add(pos)` is valid because `pos < state.position`
    // and `state.position <= state.length`, and the input buffer spans `length` bytes.
    unsafe {
        let state_ptr = lexer_state(lexer);
        if state_ptr.is_null() {
            return 0;
        }
        let state = &*state_ptr;

        // Count columns from the beginning of the current line
        let mut column = 0;
        let mut pos = state.position;

        // Go back to find the start of the line
        while pos > 0 {
            pos -= 1;
            let byte = *state.input.add(pos);
            if byte == b'\n' {
                break;
            }
            column += 1;
        }

        column
    }
}

extern "C" fn ts_lexer_is_at_included_range_start(_lexer: *const TSLexer) -> bool {
    // We don't support included ranges yet
    false
}

extern "C" fn ts_lexer_eof(lexer: *const TSLexer) -> bool {
    // SAFETY: `lexer` points to a TSLexer from `create_lexer_adapter`.
    // `lexer_state_const` is null-checked below. Shared reference is safe (read-only).
    unsafe {
        let state_ptr = lexer_state_const(lexer);
        if state_ptr.is_null() {
            return true;
        }
        let state = &*state_ptr;
        state.position >= state.length
    }
}
