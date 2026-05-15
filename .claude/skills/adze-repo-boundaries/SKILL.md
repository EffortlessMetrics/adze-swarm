---
name: adze-repo-boundaries
description: Use this when a task explicitly involves moving work between EffortlessMetrics/adze, adze-dev, and adze-swarm — promoting swarm work to public, syncing external PRs back, or migrating between repos. Do not use for normal work inside a single repo.
---

# Adze Repo Boundaries

Use this skill only when the task explicitly involves moving work between repos.

## Repos

- `EffortlessMetrics/adze` — public release and external contribution repo.
- `EffortlessMetrics/adze-swarm` — swarm working repo.
- `EffortlessMetrics/adze-dev` — contributor/dev integration repo.

## Default behavior

When inside a repo, treat it as independent. Do not mention or target another repo unless the task explicitly says to promote, sync, or migrate.

## Swarm work

Swarm work targets:

- `EffortlessMetrics/adze-swarm`

Open PRs to:

- `adze-swarm:main`

Do not open swarm PRs against public `adze`.

## Promotion

Promotion from `adze-swarm` to public `adze` happens by explicit promotion PR.

- Push to `adze:promote/<date-or-version>`.
- Open PR into `adze:main`.
- Public hosted checks gate the merge.

## Public PR backflow

External PRs land in public `adze`. Sync public `adze` back into `adze-swarm` / `adze-dev` only through explicit sync branches/PRs — never by copying files.

## Runner / CI boundary

The `em-ci-small` self-hosted runner group is scoped to `adze-swarm` only. Neither `adze` nor `adze-dev` should be added to the runner group or receive `EM_RUNNER_READ_TOKEN`. Release / publish / signing / cargo-token workflows stay on public `adze` with GitHub-hosted runners.
