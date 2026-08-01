#pragma once

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <vector>

namespace kanata::macos {

// IOHID emits one value per changed element. Values produced by the same
// physical input report share a device and timestamp, but their callback order
// is not guaranteed. Keep report and device boundaries intact while applying
// Kanata's modifier ordering rule within each report.
struct timestamped_input_event {
  uint64_t value;
  uint32_t page;
  uint32_t code;
  uint64_t device_hash;
  uint64_t timestamp;
};

inline bool is_keyboard_modifier(const timestamped_input_event& event) {
  constexpr uint32_t keyboard_page = 0x07;
  constexpr uint32_t left_control = 0xe0;
  constexpr uint32_t right_gui = 0xe7;
  return event.page == keyboard_page &&
         event.code >= left_control &&
         event.code <= right_gui;
}

inline bool is_press(const timestamped_input_event& event) {
  return event.value != 0;
}

inline void reorder_same_report_events(
    std::vector<timestamped_input_event>& events) {
  std::size_t begin = 0;
  while (begin < events.size()) {
    std::size_t end = begin + 1;
    while (end < events.size() &&
           events[end].device_hash == events[begin].device_hash &&
           events[end].timestamp == events[begin].timestamp) {
      ++end;
    }

    // Do not compare presses against releases. Instead preserve their total
    // order by sorting only contiguous runs of the same transition type.
    std::size_t transition_begin = begin;
    while (transition_begin < end) {
      std::size_t transition_end = transition_begin + 1;
      while (transition_end < end &&
             is_press(events[transition_end]) ==
                 is_press(events[transition_begin])) {
        ++transition_end;
      }
      const bool pressing = is_press(events[transition_begin]);
      std::stable_partition(events.begin() + transition_begin,
                            events.begin() + transition_end,
                            [pressing](const auto& event) {
        return pressing ? is_keyboard_modifier(event)
                        : !is_keyboard_modifier(event);
      });
      transition_begin = transition_end;
    }
    begin = end;
  }
}

} // namespace kanata::macos
