# KeyPath Fork Status

This document tracks the KeyPath-specific changes maintained in the
`keypath/bundled` branch of the `malpern/kanata` fork.

## Branch Model

```
upstream/main (jtroo/kanata)
      │
      ▼
    main ─── clean upstream mirror, never has local commits
      │
      ▼
keypath/bundled ─── main + KeyPath features cherry-picked on top
```

- **`main`** — exact mirror of `jtroo/kanata` main. Sync with
  `git fetch upstream && git merge upstream/main`. Never commit local
  features here.
- **`keypath/bundled`** — what KeyPath ships. All local features are
  cherry-picked on top of `main`. The parent KeyPath repo's submodule
  (`.gitmodules`) tracks this branch.

### Upstream sync workflow

```bash
git checkout main
git fetch upstream
git merge upstream/main
git push origin main

git checkout keypath/bundled
git rebase main                # or re-cherry-pick if conflicts are large
# resolve any conflicts
cargo build --release --target aarch64-apple-darwin
cargo build --release --package kanata-sim --target aarch64-apple-darwin
git push --force origin keypath/bundled
```

Then update the submodule pointer in the parent KeyPath repo.

## Fork Commit Inventory

### Fork-only (maintain permanently)

These are KeyPath app features with no upstream value.

| Feature | Commits | Purpose |
|---------|---------|---------|
| KeyInput TCP broadcast | 1 | Live overlay needs per-key input/release events over TCP |
| TapHoldReason tracing | 1 | HRM decision inspector in overlay UI |
| --json + canonical_key_name | 1 | KeyPath simulator test suite needs structured JSON output |

### Open Upstream PRs

These are fork features already submitted to `jtroo/kanata`. When merged,
drop from `keypath/bundled` and pick up via the next upstream sync.

| Feature | PR | Commits in fork |
|---------|-----|-----------------|
| managed-repeat + companion fixes | [#2070](https://github.com/jtroo/kanata/pull/2070) | 3 (feature, OS repeat suppression, live reload fix) |
| macos-continue-if-no-devs-found + listener fix | [#2065](https://github.com/jtroo/kanata/pull/2065) | 2 |
| cmd-fork action | [#2068](https://github.com/jtroo/kanata/pull/2068) | not in fork (separate branch) |

### Ready to File

Small changes that should be submitted as upstream PRs.

| Feature | Size |
|---------|------|
| VirtualHID wait timeout 10s to 120s | 2-line fix |
| VID:PID hex column in --list output | 1 file |

### Auto-drop on Next Sync

These disappear naturally when their parent PRs merge or upstream
catches up.

| Feature | Reason |
|---------|--------|
| Config docs for continue-if-no-devs | Ships with PR #2065 |
| Design notes for continue-if-no-devs | Internal reference only |
| clippy/rustfmt fixes (2 commits) | Upstream picks these up with Rust edition updates |

## Commit Count Trajectory

| State | Commits |
|-------|---------|
| Current | 14 |
| After open PRs merge + auto-drops | 5 |
| After filing & merging small PRs | 3 |
