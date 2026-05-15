# adze-swarm CI baseline

`adze-swarm` is the swarm working repo for Adze. It shares Adze project history and keeps the normal project-local instructions.

## Active base lane

The current durable base lane is:

- `EM CI Routed Rust`
- final check: `Rust Small Result`

This lane routes to the CX43 self-hosted runner when idle and falls back to GitHub-hosted when the runner is busy.

The inherited public `ci.yml` full-CI workflow is retained for scheduled and
manual verification only. It does not run on ordinary swarm PRs or every merge
to `main`; that keeps the swarm base loop bounded while preserving an opt-in
full-CI path.

## Public/release workflows

Release, publish, signing, Droid review, Droid security scan, and deploy-style workflows remain on public `EffortlessMetrics/adze` unless explicitly reintroduced here.

These workflows are not day-one `adze-swarm` workflows because they involve release authority, external credentials, deploy surfaces, or separate review policy.

## Staged later

The following are real CI lanes, but are staged pending routing or policy decisions:

- coverage
- benchmarks
- performance
- fuzz

They should be restored deliberately once their runner class, trigger policy, and cost profile are clear.

## Operating rule

`adze-swarm` should carry swarm-safe verification. Public release and external-contribution workflows stay on public `adze` unless explicitly promoted into this repo.

## Cutover checkpoint

New swarm work targets `EffortlessMetrics/adze-swarm`. Keep public `EffortlessMetrics/adze` side by side as the release/public-history repo, and do not retarget its `origin` remote during swarm work.

Branches for swarm tasks should be created from `adze-swarm/main`, pushed to `origin`, and opened as same-repo PRs against `adze-swarm/main`. Do not push directly to `main`, do not target `adze-dev`, and do not use `em-ci` labels for swarm routing.
