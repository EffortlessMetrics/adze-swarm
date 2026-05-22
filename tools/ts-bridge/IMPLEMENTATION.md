# ts-bridge Implementation Status

## What's Implemented (PR1)

This is the initial implementation of the Tree-sitter to GLR runtime bridge tool. 

### ✅ Completed Components

1. **Project Structure**
   - `/tools/ts-bridge` standalone crate added under `tools/`
   - FFI layer with C shim for Tree-sitter interaction
   - Schema definitions for parse table data
   - Extraction logic for converting Tree-sitter tables to our format

2. **Core Functionality**
   - `ParseTableData` schema with JSON serialization
   - Action table extraction (terminals → actions)
   - Goto table extraction (non-terminals → states)
   - Rule collection and deduplication
   - Symbol name extraction
   - Basic ABI version checking (v15)

3. **Testing**
   - Basic schema serialization tests
   - Action serialization tests
   - Parity test framework (advisory; requires actual Tree-sitter libraries)

4. **Build System**
   - C shim compilation via `cc` crate
   - Stub implementations for development
   - ABI hash checking script framework

### ⚠️ Temporary Limitations

1. **Stub Implementation**
   - Currently using stub C functions instead of real Tree-sitter
   - This allows the tool to compile and test basic functionality
   - Real Tree-sitter integration requires proper headers and libraries

2. **Missing Tree-sitter Features**
   - External scanner support not implemented
   - Field mapping (production_id → fields) stubbed for PR2
   - Lexer/tokenizer integration deferred to Track B

3. **Testing Limitations**
   - Parity tests are ignored (require actual tree-sitter-json)
   - No end-to-end testing with real grammars yet

## Next Steps (PR2 and beyond)

### PR2: Static Table Generation
- Convert `parse_table.json` → Rust static code
- Generate `static PARSE_TABLE: ParseTable` 
- Hook into GLR runtime for first real parse

### PR3: Real Tree-sitter Integration
- Link against actual Tree-sitter library
- Test with tree-sitter-json grammar
- Implement proper token/external boundary detection
- Full parity testing

### PR4: Field Support
- Wire `production_id → field_map_slices`
- Extract field names and aliases
- Support named field access in AST

### PR5: Tokenizer Integration
- Track A: Simple regex tokenizer for MVP
- Track B: FFI bridge to Tree-sitter lexer

## How to Use (Development)

```bash
# Build the tool from the repository root.
cargo build --manifest-path tools/ts-bridge/Cargo.toml

# Run tests
cargo test --manifest-path tools/ts-bridge/Cargo.toml --test basic

# Once real Tree-sitter is available:
# cargo run --manifest-path tools/ts-bridge/Cargo.toml -- path/to/libtree-sitter-json.so output.json tree_sitter_json
```

## Technical Decisions

1. **ABI Stability**: Pinned to Tree-sitter v15 with hash checking
2. **Symbol Width**: Using `u16` for all IDs with debug assertions
3. **Rule Stability**: Deterministic rule ID assignment via BTreeMap
4. **Epsilon Spans**: Will use `last_end` tracking for consistent spans
5. **GLR Precedence**: MVP ignores dynamic precedence, preserves all actions

## Architecture Notes

- **FFI Safety**: All unsafe code isolated in `ffi.rs` module
- **Data Flow**: Grammar → C extraction → JSON → Rust statics → Runtime
- **Testing Strategy**: Unit tests → Parity tests → End-to-end tests
- **CI Integration**: ABI hash checks prevent silent breakage

This implementation provides advisory foundation work for bridging
Tree-sitter-shaped table data into Adze runtime experiments. It is not a Stable
product claim or full imported-grammar compatibility promise.
