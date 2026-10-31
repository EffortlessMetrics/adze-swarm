# Developer tasks

Use the supplied Adze candidate and its public-facing documentation. Save your
commands, relevant output and changes. Stop and record an obstacle rather than
silently substituting a different tool. You may consult the documentation; record
any person/agent assistance beyond the supplied materials. Do not clean shared
caches, change global configuration, publish packages or delete existing files.

## D01 — Decide whether Adze fits

You are implementing a small Rust configuration language and need typed values.
Another component needs source ranges and syntax inspection. Explain which API
you would choose for each. Find the installation constraints and the support
limits relevant to those choices. Deliver a short explanation with documentation
references; do not rely on a repository-internal issue to make the decision.

## D02 — Build the first parser

Starting with the supplied empty working directory, follow the introductory
instructions to build and run a parser. Parse `1 + 2 * 3`, then try an invalid
expression. Show the typed result, the invalid-input response, and a passing test
for the behavior you intended. Leave the original candidate checkout unchanged.

## D03 — Use a realistic configuration

Use the supplied configuration-language example to read a policy with three
retries, two targets named `api` and `worker`, and a timeout of thirty seconds.
Show the values consumed by the application. Then omit the timeout and explain
how that differs from explicitly setting it to zero. Preserve an executable test
for both cases. Use the documented language rather than converting the input
to another configuration format before parsing it.

## D04 — Extend, break and repair the grammar

Add an optional `backoff` setting, expressed in seconds after the existing fields.
Demonstrate a nonzero value, explicit zero, absence, and continued acceptance of
old policies. Introduce a missing type in the added declaration, attempt a build,
then repair it and run the tests again. Reuse the same project and build directory
throughout; do not clean between phases. Save the source diff and outputs from
each phase. Explain how you know the result is not an old generated parser.

## D05 — Handle bad input and conversion failures

Try an empty target list, an incomplete policy, and a numeric value of `65536`
where the example expects a 16-bit unsigned value. Explain each response and
locate the offending input. Correct the inputs and show that valid values remain
unchanged. Do not turn an invalid value into a default or catch a panic and call
that a successful parse.

## D06 — Diagnose a grammar-authoring mistake

In a copy of your grammar, remove a separator between two leaf-attribute
arguments. Attempt to build it. Use the reported information to identify and
repair the declaration. Save the failed build output and the repaired test result.
Describe anything that made the error difficult to understand.

## D07 — Diagnose project setup

From an unrelated working directory, select the supplied target project rather
than the caller's project. Determine which setup checks are actually available,
what project they examined and what action is needed next. An unavailable probe
is an unavailable probe, not a passing check. Do not install tools, modify global
Cargo configuration, expose environment secrets or run arbitrary cleanup to make
the report green. Record any missing diagnostic capability as an obstacle.

## D08 — Upgrade an existing consumer

Use the supplied preserved application and its known-working prior dependency
baseline. First change only its Adze dependency selections to the candidate and
run its existing tests. Record the result. Apply only documented necessary source
changes, retain the original behavioral expectations, and show the resulting
diff and outputs. Find the rollback pin and any remaining support limitation.
Do not generate a replacement application. When no qualified prior baseline is
supplied, record that the task is blocked instead of inventing one.
