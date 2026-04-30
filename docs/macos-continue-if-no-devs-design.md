# macOS: continue-if-no-devs-found Design Notes

## Feature Summary

Allow kanata to start on macOS even when the target keyboard device is not
connected (e.g., Bluetooth keyboard not yet paired at boot). When the device
later connects, kanata grabs it and begins remapping. This mirrors the existing
Linux `linux-continue-if-no-devs-found` option.

Tracks: https://github.com/jtroo/kanata/issues/1982

## Prerequisites (all merged)

| PR | Description | Status |
|----|-------------|--------|
| [psych3r/driverkit#17](https://github.com/psych3r/driverkit/pull/17) | `device_connected_callback` for hot-plug notifications | Merged 2026-03-25 |
| [kanata#1987](https://github.com/jtroo/kanata/pull/1987) | Bump driverkit to 0.3.0 (brings in hot-plug support) | Merged |
| [kanata#1986](https://github.com/jtroo/kanata/pull/1986) | Fix wrong-device grab when include list has no matches | Merged |

## How Linux Does It

**Config option:** `linux-continue-if-no-devs-found` (boolean, default false)

**Struct field:** `Kanata.continue_if_no_devices` (line 225 in `mod.rs`)

**Parser:** `defcfg.rs:30` — `CfgLinuxOptions.linux_continue_if_no_devs_found`

**Behavior:** `KbdIn::new()` in `src/kanata/linux.rs` accepts the flag. When
true and no devices are found, it returns a `KbdIn` in a "waiting" state that
uses inotify to watch `/dev/input/` for new device nodes. The event loop
blocks on `kbd_in.read()` which internally waits for inotify events, then
attempts to grab newly appeared devices.

## macOS Architecture

### Current startup path (`src/oskbd/macos.rs:594`, `KbdIn::new()`)

```
ensure_input_monitoring_permission()
ensure_accessibility_permission()
install_karabiner_abort_handler()
driver_activated()?

enumerate devices → filter → validate_and_register_devices()

if device_names.is_empty() {
    return Err("Couldn't register any device")  ← FAILS HERE
}

grab() → seize devices + start listener thread + init sink
```

### Current C++ layer (`driverkit.cpp`)

```cpp
int grab() {
    pipe(fd);
    init_sink();                    // async Karabiner client
    fire_listener_thread();         // seize devices, start CFRunLoop
    return 0;
}
```

`fire_listener_thread()` calls `capture_registered_devices()` which:
1. Adds an IONotificationPort to the run loop (for `kIOMatchedNotification`)
2. Iterates registered device hashes and seizes matches
3. Subscribes to `device_connected_callback` per device hash

**Key insight:** The `device_connected_callback` subscription happens inside
`capture_registered_devices()`, which is called from `fire_listener_thread()`,
which is called from `grab()`. If `grab()` is never called (because no devices
were found), the callback is never registered.

### The `device_connected_callback` (driverkit.cpp:190)

```cpp
void device_connected_callback(void* context, io_iterator_t iter) {
    uint64_t device_hash = static_cast<uint64_t>(
        reinterpret_cast<uintptr_t>(context));
    for (mach_port_t curr = IOIteratorNext(iter); curr;
         curr = IOIteratorNext(iter)) {
        uint64_t curr_hash = hash_device(curr);
        if (curr_hash == device_hash)
            capture_device(IOHIDDeviceCreate(kCFAllocatorDefault, curr),
                           curr_hash);
        IOObjectRelease(curr);
    }
}
```

This callback fires when a device matching a registered hash appears in the
IOKit registry. It automatically seizes the device. **This already works for
re-connection after the initial grab** — the problem is that `grab()` must be
called first to start the listener thread and register the callbacks.

## Implementation Plan

### 1. Config option

**File:** `parser/src/cfg/defcfg.rs`

Add to `CfgMacosOptions` (line 67):
```rust
pub struct CfgMacosOptions {
    pub macos_dev_names_include: Option<Vec<String>>,
    pub macos_dev_names_exclude: Option<Vec<String>>,
    pub macos_continue_if_no_devs_found: bool,  // NEW
}
```

Add parser handling alongside the existing `macos-dev-names-include` block
(~line 664). Mirror the Linux `linux-continue-if-no-devs-found` parser pattern
at line 859.

### 2. Thread field through Kanata struct

**File:** `src/kanata/mod.rs`

Add field (near line 225, alongside the Linux equivalent):
```rust
#[cfg(target_os = "macos")]
continue_if_no_devices: bool,
```

Set it from config in the two `new()` paths (~line 475, ~line 626):
```rust
#[cfg(target_os = "macos")]
continue_if_no_devices: cfg.options.macos_opts.macos_continue_if_no_devs_found,
```

### 3. Modify `KbdIn::new()` to accept the flag

**File:** `src/oskbd/macos.rs`

Change signature:
```rust
pub fn new(
    include_names: Option<Vec<String>>,
    exclude_names: Option<Vec<String>>,
    continue_if_no_devices: bool,  // NEW
) -> Result<Self, anyhow::Error>
```

When `device_names.is_empty()` and `continue_if_no_devices` is true:
- Still call `register_device()` for the requested device names/hashes
  (even though they're not currently connected — this registers the hashes
  so `device_connected_callback` can match them later)
- Call `grab()` — this starts the listener thread and IONotificationPort
  subscriptions. The grab will seize zero devices (since none are connected)
  but the callback will fire when they appear.
- Return `Ok(Self { grabbed: false })` or a new `KbdIn` state indicating
  "waiting for devices"

**Open question:** Can `grab()` be called with registered devices that don't
exist yet? Need to verify that `capture_registered_devices()` handles the
case where `registered_devices_hashes` contains hashes that don't match any
current device — it should just skip them in the iterator and subscribe to
notifications for future matches.

### 4. Modify event loop to handle "no devices yet"

**File:** `src/kanata/macos.rs`

The event loop currently calls `kb.read()` which blocks on the pipe. If
`grab()` was called with no devices, `wait_key()` will block forever (no
events will arrive until a device connects and `device_connected_callback`
seizes it).

Two approaches:

**A. Let it block naturally:**
When a device later connects, `device_connected_callback` seizes it and
registers the input callback, which starts writing to the pipe. `wait_key()`
unblocks and the event loop proceeds normally. This is the simplest approach
but the user gets no feedback while waiting.

**B. Add a "waiting for device" state with logging:**
Before entering the main loop, if `kb.is_grabbed()` is false, log a message
and poll periodically until a device is grabbed. This gives user feedback
but adds complexity.

Recommendation: **approach A** with an info log at startup saying "no devices
found, waiting for device connection" when `continue_if_no_devices` is true.

### 5. Pass flag from event_loop caller

**File:** `src/kanata/macos.rs`

The `event_loop()` method creates `KbdIn::new()` at line 45. It needs access
to `continue_if_no_devices` from the `Kanata` struct:

```rust
let k = kanata.lock();
let continue_if_no_devices = k.continue_if_no_devices;
// ... existing code ...
let mut kb = match KbdIn::new(include_names, exclude_names, continue_if_no_devices) {
```

### 6. Documentation

**File:** `docs/config.adoc`

Add alongside the existing `linux-continue-if-no-devs-found` documentation.
Mirror the Linux docs but note macOS-specific behavior (Karabiner driver
must still be activated).

## Risks and Open Questions

1. **Can `grab()` succeed with zero matching devices?**
   The C++ `grab()` calls `init_sink()` then `fire_listener_thread()`. The
   listener thread calls `capture_registered_devices()` which iterates the
   IOKit device registry. If no registered hashes match, it captures nothing
   but still registers notification subscriptions. **Need to verify** that
   `capture_registered_devices()` returns false (no devices captured) but
   the run loop and notifications still work.

2. **Does `init_sink()` need a device to be grabbed first?**
   No — `init_sink()` starts the Karabiner virtual HID client independently
   of input devices. The sink represents the output side.

3. **What happens if `wait_key()` blocks forever?**
   If the device never connects, kanata sits idle. This is acceptable behavior
   (same as Linux). The user can Ctrl+C or the process can be killed.

4. **Recovery loop interaction:**
   The existing recovery loop in `event_loop()` handles output backend
   disconnection by releasing and re-grabbing input. If we start with no
   devices grabbed, the recovery loop should not trigger unnecessarily.
   May need to skip the `output_ready()` check when no devices are grabbed.

5. **`macos-dev-names-include` required?**
   Should `continue-if-no-devs-found` require `macos-dev-names-include` to
   be set? Without it, kanata would wait for *any* keyboard to appear, which
   is probably fine. But with an include list, kanata waits for a *specific*
   keyboard, which is the BLE use case from issue #1982.

## Test Plan

1. **Config with include list + continue flag, device disconnected:**
   Start kanata → verify it starts without error → connect BLE keyboard →
   verify kanata grabs it and remapping works

2. **Config with continue flag, no include list:**
   Start kanata with no keyboards → verify it waits → plug in USB keyboard →
   verify grab

3. **Device disconnect/reconnect cycle:**
   Start with device → disconnect → verify keyboard works (unseized) →
   reconnect → verify kanata re-grabs (existing recovery path)

4. **Without continue flag (existing behavior preserved):**
   Start without device → verify kanata fails with existing error message
