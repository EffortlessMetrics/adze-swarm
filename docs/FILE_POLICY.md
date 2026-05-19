# File Policy

Adze is a Rust-first project. Rust source and `xtask` automation are the
default implementation surfaces. Every other file type — YAML, JSON, shell,
Markdown, HTML, language fixtures, generated grammars — must be **registered**
in [`policy/non-rust-allowlist.toml`](../policy/non-rust-allowlist.toml).

## Why register

A registry forces three things:

1. **Surface awareness.** Every non-Rust artifact has an explicit owner.
2. **Replacement candor.** The `reason` field describes why a non-Rust
   artifact is required (platform constraint, fixture corpus, generated
   asset, etc.).
3. **Coverage.** The `covered_by` field names the commands or workflows
   that actually exercise the artifact.

Without this, non-Rust files accumulate as "tribal knowledge" with no
ownership and no test coverage.

## Schema

Each `[[allow]]` entry must include:

| Field            | Required | Notes                                               |
| ---------------- | -------- | --------------------------------------------------- |
| `glob`           | yes      | repo-relative globset pattern (alternatively `path`) |
| `kind`           | yes      | category, e.g. `ci_declarative`, `language_fixture` |
| `owner`          | yes      | team or code area                                   |
| `surface`        | yes      | ci/docs/fixtures/editor/grammar/build/…             |
| `classification` | yes      | config/docs/fixtures/test/tooling/generated/production |
| `reason`         | yes      | one-liner: why is this not Rust?                    |
| `covered_by`     | yes      | list of commands/workflows that exercise it         |
| `expires`        | optional | ISO date for revisit                                |
| `retired`        | optional | `true` if the entry only exists for audit history   |
| `generated_by`   | optional | for `classification = "generated"` entries          |

## Workflow

```bash
# Fail-fast check — reports any non-Rust file outside the allowlist.
cargo xtask check-file-policy

# Reports also include unused entries; use that to keep the registry clean.
```

The check runs in **advisory** mode for now: it writes
`target/policy/file-policy.md` and `target/policy/file-policy.json` but does
not fail CI. Once the baseline is settled, this will flip to blocking.

The same report includes a **Rust migration candidates** section. These are
non-Rust files that are executable repository logic, production grammar
definitions, or durable tooling surfaces that should move toward `xtask`, a
Rust grammar crate, or an owner module when touched. Fixtures, generated files,
docs, and platform-required configuration remain registered without becoming
migration targets.

## Adding a new non-Rust file

1. Try Rust first. Many YAML configs and shell scripts can be replaced with
   an `xtask` subcommand. Prefer that.
2. If a non-Rust file is genuinely required (platform constraint, fixture
   input, generated artifact), add a `[[allow]]` entry to
   `policy/non-rust-allowlist.toml` with all required fields.
3. Run `cargo xtask check-file-policy` to verify it matches.

## Generated artifacts

Files matched by a `classification = "generated"` entry must also set
`generated_by` to the command that produces them. The check reports
generated files as part of its inventory; future iterations will verify
that the generating command exists in the xtask graph.
