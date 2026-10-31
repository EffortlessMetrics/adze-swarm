# Developer-task evaluation packet, version 1

Owner: #1140. Developer journey: #1128. Product acceptance remains #853.

This packet implements task instructions, an evaluator rubric and a local
observation template. **No developer evaluation has been performed by adding
these files. Human usability is not evaluated.** Validator unit tests use
synthetic records and are not participant sessions or parser proof.

## Prepare one run

Select an exact candidate and the exact documentation commit. Use a qualified
candidate that includes the tasks being evaluated. Freeze the supplied build,
docs and task packet for that run; do not credit later fixes retroactively.
A task whose prerequisite is unavailable remains blocked, with its owner named.
In particular, do not present the unqualified configuration example or a missing
upgrade baseline as already usable. Source-checkout, local-registry and public
registry journeys must be distinguished in the evidence descriptions.

Give the participant only [tasks.md](tasks.md) and the selected public-facing
README, tutorial, API reference and sample documentation. Keep
[evaluator.md](evaluator.md) separate. Do not provide internal issue threads,
solutions, hidden terminal history or the evaluator's expected values as hints.
An agent rehearsal must use an independent context without the implementation
conversation. Label it `agent`; it cannot establish human satisfaction.
Human sessions require consenting real participants; this packet authorizes
neither recruiting nor telemetry. Use anonymous participant codes.

Copy [observation-template.json](observation-template.json) outside tracked source.
Replace every identity/timestamp placeholder and record all eight task rows.
The untouched template deliberately fails validation: it is not a completed run.
Keep secrets, personal names, environment dumps and unnecessary absolute paths
out of observations and attached logs. Retain useful artifact references with
source and command context; do not paste credentials into the repository.

From the repository root:

```sh
python3 scripts/check_developer_observation.py path/to/observation.json
python3 scripts/test_developer_observation.py -v
```

The checker is offline and read-only. Exit 0 means only that the observation is
structurally consistent. Exit 2 means malformed/incomplete data or a read error.
It does not execute tasks, fetch artifacts, verify commits, authenticate a human,
prove consent, validate the claimed output, or certify product readiness. The
reviewer must verify those facts against the artifacts and rubric. Do not wire
this command as a parser proof gate or add its test counts to product receipts.

## Turn findings into repairs

Prioritize lost files, accepted wrong values and stale output; then blocked
onboarding, unclear errors, slow iteration and discoverability. Record one
minimal reproduction under the existing owner in the rubric. Use a new bounded
issue only when the defect has no owner. Keep before/after task observations at
their own candidate identities and retain assistance instead of relabeling a
helped completion as independent. Do not compute an adoption or satisfaction
percentage from this packet. Missing human participation is not a reason to
pause independent correctness or documentation work.
