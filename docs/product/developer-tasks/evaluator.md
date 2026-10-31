# Evaluator rubric — packet version 1

Do not supply this file as participant instructions. Owner: #1140; journey #1128.
The rubric is task-level evidence, not a new release or parser-proof authority.

## Before starting

Record exact source/docs commits, packet version, toolchain and platform. Verify
that the installed binary or source candidate is the selected one. Read actual
PR/issue state before describing a prerequisite as implemented. Prepare a clean,
owned working directory and file-preservation sentinels. Do not reuse private
solution context in an independent agent rehearsal. Record human consent locally
before a human session; store no identifying consent document in this repository.

No warm-up answer may be counted as an unassisted task. Freeze task instructions
and rubric before the run. A fix creates a new run; it does not alter old evidence.

## Independent acceptance and ownership

| Task | Observable acceptance, including negative controls | Canonical owner |
| --- | --- | --- |
| D01 | Chooses typed parsing for typed values and document APIs for syntax/source facts, locates exact installation/support boundaries. References the supplied docs, not internal hints. | #873 / #1128 |
| D02 | Exact precedence AST `Add(Number(1), (), Mul(Number(2), (), Number(3)))`; invalid expression rejected with a useful source-bound diagnostic; the named test actually executes. Installed/source-path scope stays explicit. | #874 / #864 / #880 |
| D03 | Retry value `3`, target order `[api, worker]`, timeout `Some(30)`; independent assertions distinguish absent timeout from `Some(0)`. CLI and application output agree. | #1129 / #876 / #1062 |
| D04 | Original grammar rejects the extension; edited grammar returns new values while preserving old ones; the deliberately missing type causes the expected failure; repair executes the new parser without cache deletion. Original files/sentinels are preserved. | #1138 / #1067 |
| D05 | Empty list/incomplete input rejected; `65536` is a conversion error with its actual token range, not panic/default success; nearby valid maximum remains `65535`. No ignored-only or zero-test proof. | #875 / #864 / #898 |
| D06 | Causal grammar-authoring diagnostic names the offending declaration at its source; repair restores the valid behavior. A Cargo network error is not this negative control. | #1130 / #1141 |
| D07 | Explicit target, not caller directory, governs available checks. Missing/unknown/error states are distinguished. No build script, network, installation, mutation or secret dump is hidden inside a claimed read-only probe. An absent doctor command is blocked functionality, not a participant failure. | #1131 / #1073 |
| D08 | A qualified preserved baseline is run first; dependency-only upgrade is attempted before any migration; necessary edits are documented and exact old assertions are retained. New starter generation cannot qualify. | #1139 / #1072 |

Where a candidate is missing functionality, retain `blocked` and the owner above.
Do not route around a valid construct, weaken an oracle, or substitute a different
parser just to call the task complete. Existing merged fixes stay credited only
at the identities where they were actually tested.

## Record the observation

All eight rows remain present. Use `not_attempted` with a reason for skipped tasks,
`blocked` with a concrete cause/owner, `assisted` with the actual help, or
`independent` only when no external solution/help was supplied. Normal use of the
supplied public documentation is not assistance. Record failed commands and
confusing messages even if the task eventually succeeds. Completion needs the
actual value/artifact and elapsed time to the first correct result, not a statement
that the task is done. Retrying after assistance does not erase the assistance.

Evidence references should identify the phase, command, exit, candidate and
observable output; use retained, appropriately redacted artifacts. A reviewer
must inspect them. The JSON checker validates neither their existence nor their
truth. Record a toolchain identity reported by the actual environment, never the
version the guide merely requests. Separate command time from human task time.

After a blocked task, establish a fresh baseline for later tasks. Do not silently
credit evaluator repairs to the participant. No unauthorized recruiting, telemetry,
automatic upload, publication, or global machine change is part of this packet.

## Review and retest

Report observed task outcomes and concrete friction, not a satisfaction score or
an extrapolated adoption percentage. Distinguish agent-only observations from
human observations in every summary. Human usability remains `not evaluated`
until actual consenting participants have completed sessions. Map repeated
obstacles to existing owners, retain an executable reproduction where possible,
and retest the unchanged task/rubric on the repaired candidate. A passing
observation-schema test does not close #1140, #1128 or #853.
