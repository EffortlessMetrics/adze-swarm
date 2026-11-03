# Generated canonical field metadata

The native `TSLanguage` parsing ABI does not contain enough information to
recover canonical field targets or cardinality. Alias sequences and decoded
production placeholders are not substitutes for the complete grammar.

The pure Rust generator emits `LANGUAGE_DESCRIPTOR` beside `LANGUAGE`. Its
`GeneratedFieldSchemaV1` contains the owner, field name, required/multiple facts
and public grammar target IDs produced by
`adze_tablegen::field_schema::field_semantics_for_grammar` while the prepared
complete grammar is available. This is a transport projection of that authority,
not another field analysis. The native ABI layout is unchanged.

The descriptor carries its own format version and canonical semantic revision.
Neither number is the native ABI version or a whole-grammar fingerprint. Before
parsing, the descriptor-aware entry checks that the schema belongs to the exact
native language object, that public-symbol and field-name table snapshots agree,
and that facts reference known owners, fields and targets without duplicate
owner/field entries. As with parsing itself, native pointers must obey the
existing immutable static `TSLanguage` table contract. Validation is metadata
pairing, not validation of pointers fabricated by unsafe code.

Generated `parse_document` binds its descriptor automatically. Both deterministic
LR and GLR document construction receive the same validated schema source,
including documents containing diagnostics or synthetic failure roots. Consumers
can inspect `LanguageMetadata::field_schema_availability()` and use
`checked_field_semantics(owner, field)` to distinguish an absent grant from
unavailable canonical metadata. The compatibility `field_semantics` accessor
returns `None` for either case.

Callers that supply only a native language through `__private::parse_document`
or the raw streaming materialization helper get `Unavailable` schema metadata.
Their field-name table remains usable for traversal. `validate_fields` returns
`FieldValidationError::SchemaUnavailable`, even if the selected tree has no
field-bearing edges. It must not infer a negative field grant or successful
validation from absent canonical facts. Explicit complete-IR document
construction continues to obtain facts from the canonical authority; callers
supplying IR are responsible for its completeness.

This first transport slice proves the actual generated Pair grammar's owner 4,
left target 5 and right target 6, both required and singular, and a generated
conflicted grammar dispatched through streaming GLR. It also checks descriptor
identity, table mismatch rejection, and available/unavailable metadata on clean
and error documents. It does not change the canonical analyzer's treatment of
optional/repeated/mixed alternatives or aliases, add target/cardinality checking
to the owner-grant validator, or qualify the remaining #1165 feature matrix.
ABI-only compatibility adapters need a future explicit descriptor-aware entry
before they can claim canonical schema availability.

`GeneratedLanguageDescriptor::validate` exposes structured
`GeneratedSchemaError` failures. The current private document entry retains its
existing `Result<AdzeDocument, Vec<ParseError>>` compatibility signature and
encodes descriptor rejection as `UnexpectedToken` at 0..0. That error describes
metadata setup, not an offending input token; callers needing the exact failure
class should validate the descriptor directly. A dedicated parser setup error
channel remains diagnostic-class work under #875/#1165.

## Bounded node-scoped strict validation

`AdzeDocument::validate_fields_strict_at(owner)` validates one selected owner
through the same producer program and runtime observation index used by checked
typed-CST handles. It does not recursively validate every field-owning descendant
or change the legacy grant-only `validate_fields()` operation.

Projection transport layout remains version 1. Interpretation revision 2, emitted
by the shared tablegen authority, additionally certifies direct FIELD observations
for admitted single-rule owners without Optional transitions. Revision 1 retains
its positional Optional interpretation. Complete-IR construction compiles the
same authority into the same runtime interpreter; fact-only descriptors without
a bound projection remain explicitly unavailable.

The bounded strict operation reports `WrongTarget` with the actual offending
document-local NodeId and grammar identity, `MissingRequired`, or `TooMany` with
all actual occurrence NodeIds in source order. Correct grants alone do not certify
a positional path: mismatched unfielded slots or field tags remain incomplete.
Recovered owners remain incomplete. Explicit producer Optional transitions retain
present/absent meaning without descendant search or helper-name inference.

This slice covers the generated no-alias Pair, annotated and explicit-IR Optional
present/absent controls, and direct-owner malformed/cardinality controls. Wider
repeat/nullable occurrence, ambiguous selected alternatives, native LR aliases,
query/compatibility field-path integration and genuine-RHS extras remain separate
acceptance obligations. It does not establish whole-document strict validation
or close the parent shared-schema requirements.
