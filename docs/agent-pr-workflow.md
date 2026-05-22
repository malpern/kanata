# Agent PR Workflow

End-to-end process for shepherding code from initial request through to a clean merge. Every agent working on code changes must follow this workflow. The goal is zero lost work and a clean repo state at every stage.

## Branch Model

This repo has two long-lived branches with **diverged histories**:

- **`main`** — tracks upstream `jtroo/kanata`. Synced with `git fetch upstream && git merge upstream/main`.
- **`keypath/bundled`** — the shipping branch for KeyPath. Contains fork-only commits not in upstream.

Most PRs target `main`. Changes that need to ship in KeyPath are then cherry-picked onto `keypath/bundled`. Some PRs (fork-only features) target `keypath/bundled` directly.

**Never merge one branch into the other.** Cherry-pick only.

## Phase 1: Setup

1. **Enter a worktree** — isolates your work from the user's working copy and other parallel agents. Verify you're in the worktree before editing files.
2. **Verify build** — `cargo build` must succeed before you start editing. Catches environment issues early.

## Phase 2: Development

3. **Do the work** — edit code, iterate with the user.
4. **Build** — `cargo build` must pass before any commit.
5. **Test** — `cargo test -p kanata-tcp-protocol` must pass. Never commit code that breaks tests.
6. **Commit** — commit frequently as you work. Use descriptive messages. Include `Co-Authored-By` tag.

## Phase 3: PR Creation

7. **Squash commits** — `git reset --soft <target-branch> && git commit` with a comprehensive message covering all changes.
8. **Push the branch** — `git push -u origin <branch-name>`.
9. **Link issues** — check if any open GitHub issues are addressed by this work (`gh issue list --state open`). Include `Fixes #NNN` in the PR body for each one so GitHub auto-closes them on merge.
10. **Create the PR** — `gh pr create` with a summary, `Fixes` references, and test plan. Return the URL to the user.

## Phase 4: Babysit the PR

11. **Wait for CI** — poll `gh pr checks <number>` until all checks complete. Don't guess — wait for actual results.
12. **Address review comments** — read `gh api repos/<owner>/<repo>/pulls/<number>/comments`, fix each issue, amend the commit, force-push.
13. **Resolve conflicts** — if the target branch has moved ahead, rebase or merge, resolve conflicts, push.
14. **Re-check CI** — after any push, wait for all checks to go green again.
15. **Repeat 11–14** until all checks pass and no unaddressed review comments remain.

## Phase 5: Merge

16. **Ask the user for permission to merge** — never merge without explicit approval.
17. **Merge** — `gh pr merge <number> --squash --delete-branch`.
18. **Verify the merge** — `gh pr view <number> --json state` should show `"state": "MERGED"`.
19. **Verify issues closed** — for each `Fixes #NNN` reference, confirm the issue is now closed.

## Phase 6: Cleanup

20. **Exit the worktree** — `ExitWorktree` with `action: "remove"` and `discard_changes: true` (safe because all work is merged).
21. **Pull the target branch** — `git checkout <target> && git pull`. Verify it fast-forwards to include your merged PR. If it doesn't fast-forward, investigate.
22. **Confirm to the user** — state explicitly: PR merged, issues closed, branch pulled, worktree cleaned up.

## Phase 7: Cherry-Pick to keypath/bundled (when needed)

If the merged change also needs to ship in KeyPath:

23. **Cherry-pick** — `git checkout keypath/bundled && git cherry-pick <merge-sha>`. Resolve conflicts if needed.
24. **Build** — `cargo build` must pass.
25. **Test** — `cargo test -p kanata-tcp-protocol` must pass.
26. **Push** — `git push origin keypath/bundled`. The pre-push hook verifies this is a fast-forward. If it rejects the push, something is wrong — investigate, don't override.
27. **Confirm** — tell the user the cherry-pick landed on `keypath/bundled`.

## What Can Go Wrong

| Symptom | Cause | Prevention |
|---------|-------|------------|
| Work "disappears" after merge | Merged to GitHub but never pulled locally | Always do Phase 6 step 21 |
| Issues stay open after merge | PR body didn't include `Fixes #NNN` | Step 9: link issues before creating the PR |
| PR shows conflicts after merge | Another PR merged first | Resolve conflicts before merging (step 13) |
| Cherry-pick breaks keypath/bundled | Didn't build/test after cherry-pick | Phase 7 steps 24–25 are mandatory |
| Force-push destroys keypath/bundled | Merged main into bundled or force-pushed | Pre-push hook blocks this; never attempt it |
| Worktree left behind | Agent exited without cleanup | Always exit worktree on completion |

## Non-Negotiable Rules

- **Never merge without user permission.** Even if CI is green and reviews are addressed.
- **Never force-push to `keypath/bundled` or `main`.** Only force-push to feature branches during PR iteration.
- **Never merge `main` and `keypath/bundled` in either direction.** Cherry-pick only.
- **Never leave a worktree behind.** Clean up on completion or abandonment.
- **Always verify CI after every push.** Don't assume a previous green carries forward.
- **Always link issues.** If the work addresses an open issue, the PR must include `Fixes #NNN`.
- **Always build and test before pushing to `keypath/bundled`.** The pre-push hook catches force-pushes, not broken code.
