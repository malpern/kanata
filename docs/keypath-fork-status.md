# KeyPath fork status

## Current-upstream candidate

The KeyPath fork candidate is based on upstream Kanata commit `b05b63d`
(`upstream/main`, 2026-07-29) and lives on the local branch
`codex/keypath-bundled-current-upstream`.

The macOS backend now consumes the external `karabiner-driverkit` 0.4.0 / 
VirtualHIDDevice v8 architecture introduced upstream. KeyPath's DriverKit
changes are therefore maintained in the separate local branch
`codex/macos-iohidqueue-current-upstream` in the `driverkit` repository:

- `a71a5f4` preserves physical HID report boundaries with `IOHIDQueue`.
- `9fcaf6f` preserves output report state and resets the virtual keyboard after
  a DriverKit reconnect.

For local validation, Cargo selects that checkout with:

```sh
cargo test --workspace \
  --config 'patch.crates-io.karabiner-driverkit.path="../driverkit-iohidqueue-upstream"'
```

Before this candidate replaces `keypath/bundled`, the DriverKit dependency must
be pinned reproducibly to a reviewed fork commit or an accepted upstream
release. Do not leave the production build dependent on a sibling checkout.

## Retained KeyPath behavior

The current-upstream candidate retains the KeyPath-specific behavior still
needed by the app:

- managed repeat and recovery-safe repeat allowlisting;
- structured simulator JSON and canonical key names;
- the longer VirtualHID readiness timeout and VID:PID list output;
- TCP events for key input, tap/hold reasoning, chord/tap-dance resolution, and
  authoritative input-grab state;
- the Caps Lock LED-sync revert required for Caps Lock remapping;
- processing-latency recovery, stuck-key cleanup, and device-name sanitizing;
- macOS report-boundary preservation for modifier ordering.

The old local `macos-continue-if-no-devs-found` stack and its listener fix were
not replayed because upstream PR #2065 is now present in `upstream/main`.
Likewise, the old vendored DriverKit 0.3.1 tree was not copied into Kanata;
equivalent still-required behavior was ported to the current external
DriverKit 0.4.0 source instead.

## Validation

Completed locally on macOS:

- native report-ordering test with AddressSanitizer and UndefinedBehaviorSanitizer;
- `karabiner-driverkit` build and tests;
- full Kanata workspace: 514 passed, 4 intentionally ignored, 0 failed;
- release `aarch64-apple-darwin` Kanata and simulator builds.

Release artifact hashes from the validated build:

- Kanata: `447a6d2b3f2fe0e15e0c88e24558e2ce88b4b356730a285e00ab177dc0ae5d79`
- Kanata simulator: `90d88c7dc0a188dfa15141170011d42ee660b377e5431d5846554262eb009da0`

Physical acceptance remains intentionally separate: run an unpatched
current-upstream control followed by the patched candidate through the strict
ESP32/Jig matrix during an exclusive desktop window. No upstream pull request
should be opened until those results and this patch stack have been reviewed.
