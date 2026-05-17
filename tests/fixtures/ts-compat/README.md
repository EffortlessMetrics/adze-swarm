# Tree-sitter Compatibility Fixtures

Tree-sitter compatibility fixtures prove selected-tree traversal, identity,
fields, node-types metadata, S-expression output, and error/missing node
projection for the documented subset.

Compatibility fixtures must read from `AdzeDocument` facts. They must not encode
local semantics that should belong to the document or language schema.
