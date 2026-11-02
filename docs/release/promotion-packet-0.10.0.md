# 0.10.0 Promotion Packet

> **This packet does NOT authorize promotion or publication.**
> It is evidence assembly only. Promotion and crates.io publication
> require separate explicit decisions under #325.

Machine-readable twin: `promotion-packet-0.10.0.json` (generated from the same typed packet; do not hand-edit either file).

**Candidate status:** usable-core candidate, #853 not complete

## Identity

| Field | Value |
|---|---|
| Version | 0.10.0 |
| Selected candidate commit | `e0c2a316f158eb33732623dd7198de421c26bf7f` |
| Selected candidate tree | `1d7d3030fbc7bb56c63eb2e56034b73fd66ef438` |
| Packet commit | `0451427cc2dabf24b15e102e27368d2938f6569c` |
| Public target commit | `6263c6a80046d13fb98e3ad319dfe726f32f1010` |
| Public target tree | `565e5ea130a18cb3afcd2bbc03156de228cb8cba` |
| Cargo.lock SHA-256 | `7d78a208a886be91c5f3ddb0a77c3b5fc54efb33bd13936aafc1b24df36185f8` |
| Roadmap counts | uncomputed |

## Release graph

Publish in this exact order (from `policy/release-graph.toml`):

1. `adze-bdd-governance-core` 0.10.0
2. `adze-common-type-ops-core` 0.10.0
3. `adze-common` 0.10.0
4. `adze-ir` 0.10.0
5. `adze-glr-core` 0.10.0
6. `adze-linecol-core` 0.10.0
7. `adze-macro` 0.10.0
8. `adze-parsetable-metadata` 0.10.0
9. `adze-tablegen` 0.10.0
10. `adze` 0.10.0
11. `adze-tool` 0.10.0
12. `adze-cli` 0.10.0

## Receipts

No receipts attached.

## Explicit non-claims

- This packet does not authorize promotion or publication; #325 still owns authorization.
- #853 is not complete; this packet claims a usable-core candidate only.
- Roadmap/gate counts were not recomputed from the live GitHub issue graph; see roadmap_counts (uncomputed).
- No crates.io publication, tag, or signing is claimed by this packet.
- journey (local-registry package-first install): no durable receipt attached

## Regeneration

Packet commit above records the generation base (HEAD at generation time) as provenance only. `--check` compares all generated fields and files byte-wise except the Packet commit line in this Markdown and the corresponding JSON field; every other difference is a failure.

```bash
cargo run -q -p xtask -- promotion-packet --source e0c2a316f158eb33732623dd7198de421c26bf7f --public-target 6263c6a80046d13fb98e3ad319dfe726f32f1010 --version 0.10.0 --output docs/release
cargo run -q -p xtask -- promotion-packet --source e0c2a316f158eb33732623dd7198de421c26bf7f --public-target 6263c6a80046d13fb98e3ad319dfe726f32f1010 --version 0.10.0 --check
```
