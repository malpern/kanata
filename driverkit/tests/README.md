# macOS input-ordering test

This focused native test covers the ordering policy used when an `IOHIDQueue`
drain contains multiple changed elements. Only events with the same device and
HID timestamp may be reordered. Modifier presses move before ordinary presses,
modifier releases move after ordinary releases, and report boundaries remain
intact.

Run it on macOS with:

```sh
clang++ -std=c++20 -Wall -Wextra -Werror \
  driverkit/tests/input_event_ordering_test.cpp \
  -o /tmp/kanata-input-event-ordering-test
/tmp/kanata-input-event-ordering-test
```

For an additional memory- and undefined-behavior check, add
`-fsanitize=address,undefined -fno-omit-frame-pointer` to the compile command.
