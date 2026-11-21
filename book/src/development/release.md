# Release Process

Adze prepares release candidates in
[adze-swarm](https://github.com/EffortlessMetrics/adze-swarm). Public promotion
and release happen separately in [adze](https://github.com/EffortlessMetrics/adze).
A passing preflight is not permission to promote, tag, sign, or publish.

## Prepare a Release Candidate

Use the [release-candidate bundle checklist](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/reference/RELEASE_CANDIDATE_BUNDLE.md)
and the [promotion packet tracker (#881)](https://github.com/EffortlessMetrics/adze-swarm/issues/881)
to record the selected swarm commit and tree, the intended public target, the
package and API differences, and the proof for that candidate. Record known
gaps alongside the proof.

The [package-first portability tracker (#880)](https://github.com/EffortlessMetrics/adze-swarm/issues/880)
tracks Linux and Windows verification of the packaged starter from the same
candidate. A source-built starter or an isolated local-registry check does not
verify a crates.io install.

## Check the Crate Graph

The committed [release graph](https://github.com/EffortlessMetrics/adze-swarm/blob/main/policy/release-graph.toml)
defines which crates are in the release and their dependency order. It is
generated from the [package-boundary ledger](https://github.com/EffortlessMetrics/adze-swarm/blob/main/policy/package-boundary.toml).
The [derived crate list](https://github.com/EffortlessMetrics/adze-swarm/blob/main/scripts/release-crates.txt)
is generated too. Do not edit it by hand.

## Promote or Publish

An approved candidate is promoted to public `adze` through a separate PR.
[Release decision tracker #325](https://github.com/EffortlessMetrics/adze-swarm/issues/325)
records explicit maintainer decisions and the post-publish install receipt;
the issue itself is not authorization to act.

For an authorized release, use the [publish checklist](https://github.com/EffortlessMetrics/adze-swarm/blob/main/docs/reference/PUBLISH_CHECKLIST.md)
from the promoted public state. Verify the real crates.io install for the
published version before claiming that the published starter works.
