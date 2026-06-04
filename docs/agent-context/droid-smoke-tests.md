# Droid Smoke Test Checklist

Live validation steps for Droid review workflows in this repository.

## Pre-Deployment Validation

Before declaring the Droid setup production-ready, manually verify:

### 1. Workflow Syntax
```bash
# Check YAML validity
cat .github/workflows/droid*.yml | yq . > /dev/null
```
Expected: No YAML parse errors.

### 2. Action Reference Audit
```bash
grep -r "Factory-AI/droid-action\|droid-action@main\|droid-action@v5" .github/workflows/
```
Expected: No output (no unsafe refs).

### 3. Safe Configuration Check
```bash
grep "upload_debug_artifacts: false" .github/workflows/droid*.yml
grep "EffortlessMetrics/droid-action-safe@01e76b659e4b1e5f23feedc8cfabf8dc14c7485f" .github/workflows/droid*.yml
```
Expected: Every Droid action use pinned to safe ref with artifacts disabled.

## Live Smoke Test (Same-Repo PR)

1. **Create a draft PR** on the same repository (not a fork)
   ```bash
   git checkout -b droid-smoke-test
   echo "# Smoke test" >> README.md
   git commit -am "test: droid smoke test"
   git push -u origin droid-smoke-test
   # Open PR via GitHub UI — mark as draft
   ```

2. **Confirm draft PRs are skipped**
   - `Droid PR Review` job is skipped while the PR is draft
   - No Factory or MiniMax secret-backed step runs in the draft state

3. **Convert PR to ready for review**

4. **Confirm Droid Auto Review starts**
   - Workflow logs show "Run Droid Auto Review" step completed
   - No skipped steps due to missing FACTORY_API_KEY
   - Fork PR guard did not skip the same-repo PR

5. **Verify MiniMax model execution**
   - Logs show "custom:MiniMax-M3-0" in model selection
   - No Factory default model fallback

6. **Confirm no raw artifacts uploaded**
   - Artifacts tab in workflow run shows no artifacts
   - Specifically, no `droid-review-debug-<run_id>` artifact
   - No `.factory/**` files exposed

7. **Review comment validation**
   - Comment posted to PR
   - Uses inspection-record format if no findings
   - Finding format is [P0|P1|P2] structured if issues found
   - No raw JSON or debug output in comment

8. **Confirm fork guard**
   - Fork PRs skip before the Factory/MiniMax secret-backed action step
   - Guard is equivalent to `github.event.pull_request.head.repo.full_name == github.repository`

## Manual @droid Trigger Test

1. **Comment as repository owner/member**:
   ```
   @droid review
   ```
   - Job should trigger (check "Droid Exec" logs)
   - Should produce same structured review as auto-review
   - Can run alongside auto-review without conflicts

2. **Comment as public user** (if accessible):
   ```
   @droid review
   ```
   - Job should NOT trigger (workflow should skip due to guard)
   - Logs should show guard condition failed

## Security Scan Test

1. **Manual trigger via `workflow_dispatch`**:
   - GitHub Actions tab → Droid Security Scan
   - Click "Run workflow"
   - Select branch
   - Confirm execution with MiniMax model

2. **Scheduled execution**:
   - Workflow is set to run Monday 8 AM UTC
   - Will create issues or comments for findings

## Model Provider Validation

1. **Check MiniMax usage in provider dashboard**:
   - Log into MiniMax account
   - Verify API requests appear under MINIMAX_API_KEY
   - Confirm usage volume aligns with review triggers

2. **BYOK settings file structure**:
   ```bash
   # Verify in workflow logs during Configure step
   grep -A 10 "customModels" "$RUNNER_TEMP/minimax-m3-settings.json"
   ```
   Expected:
   - `apiKey: ${MINIMAX_API_KEY}` (literal dollar-brace, not expanded)
   - `baseUrl: https://api.minimax.io/anthropic`
   - `provider: anthropic`
   - `model: MiniMax-M3`
   - the action receives this file through its `settings` input and merges it into `$HOME/.factory/droid/settings.json`

## Post-Smoke Sign-Off Checklist

- [ ] Auto-review works on same-repo PR
- [ ] Draft and fork PRs skip before Factory/MiniMax secrets are used
- [ ] Review uses MiniMax M3 model
- [ ] No raw Droid debug artifacts uploaded
- [ ] Clean review uses inspection-record format
- [ ] Finding format is [P0|P1|P2] structured
- [ ] Manual @droid review works as OWNER/MEMBER/COLLABORATOR
- [ ] Public user cannot trigger @droid (guard works)
- [ ] Security scan trigger works and uses MiniMax
- [ ] MiniMax dashboard shows API requests
- [ ] No Factory-AI/droid-action refs in workflows

## Cleanup After Smoke Test

```bash
# Delete smoke test branch
git push origin --delete droid-smoke-test
git branch -d droid-smoke-test
```

## Troubleshooting

### Workflow skipped due to missing FACTORY_API_KEY
- Verify secret exists in Settings → Secrets and variables → Actions
- Ensure it's set at repository level (not just org level for this phase)

### No review comment posted
- Check job logs for Factory API errors
- Verify MINIMAX_API_KEY is set and valid
- Confirm MiniMax account has available credits/quota

### Raw artifact appears despite `upload_debug_artifacts: false`
- This indicates an action upgrade or misconfiguration
- Audit workflow YAML for any calls to upload external artifacts
- Report to deployment checklist owner

### @droid mention doesn't trigger
- Verify commenter is OWNER/MEMBER/COLLABORATOR
- Check workflow logs for guard condition evaluation
- Ensure `github.event.comment.author_association` is populated
