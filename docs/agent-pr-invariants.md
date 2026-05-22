# Agent PR Invariants

What must be true at each phase boundary. The agent's job is to make these true and verify they are — the specific commands are implementation details.

## After Setup

- [ ] Working in an isolated worktree (not the user's checkout)
- [ ] `cargo build` succeeds from the worktree

## After Development

- [ ] `cargo build` passes
- [ ] `cargo test -p kanata-tcp-protocol` passes (zero failures)
- [ ] Changes are committed with descriptive messages

## After PR Creation

- [ ] Single squashed commit on the branch
- [ ] Branch pushed to origin
- [ ] PR exists on GitHub with summary and test plan
- [ ] Every open issue addressed by this work has `Fixes #NNN` in the PR body

## After Babysitting

- [ ] All CI checks green
- [ ] Zero unaddressed review comments
- [ ] No merge conflicts with the target branch
- [ ] User has approved the merge

## After Merge — The Completion Gate

All of these must be true before the agent reports "done." If any fails, fix it before proceeding.

```
PR state == MERGED
local target branch SHA == origin target branch SHA
linked issues state == CLOSED
no worktrees left for this branch
```

The agent must verify each assertion, not assume prior steps succeeded. A single verification block at the end catches everything — skipped steps, failed pushes, stale state.

## Cherry-Pick Gate (keypath/bundled)

When a merged PR also needs to land on `keypath/bundled`, these additional invariants apply:

```
cherry-pick applied cleanly to keypath/bundled
cargo build passes on keypath/bundled
cargo test -p kanata-tcp-protocol passes on keypath/bundled
push to keypath/bundled is fast-forward (pre-push hook enforces this)
```

Never cherry-pick onto `keypath/bundled` without building and testing first. The pre-push hook blocks force-pushes, but a bad cherry-pick that builds broken code is just as dangerous.

## Why Invariants Over Checklists

Agents lose context in long conversations. A step-by-step checklist works when followed perfectly but fails silently when a step is skipped. Invariants are self-healing: the verification gate at the end catches any gap regardless of how the agent got there.

The procedural workflow (`agent-pr-workflow.md`) is a reference for the *typical* path through these phases. But the invariants are what matter — they define "done" unambiguously.

## Non-Negotiable Rules

- **Never merge without user permission.**
- **Never force-push to `keypath/bundled`.** The pre-push hook blocks this, but don't even try.
- **Never force-push to `main`.** Only force-push to feature branches during PR iteration.
- **Never merge `main` into `keypath/bundled` or vice versa.** They have diverged histories. Cherry-pick only.
- **Always link and verify issues.**
- **Clean up worktrees.** No orphaned branches or directories.
