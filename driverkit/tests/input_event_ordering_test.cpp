#include "../c_src/input_event_ordering.hpp"

#include <cassert>
#include <cstdint>
#include <iostream>
#include <vector>

namespace {

using kanata::macos::reorder_same_report_events;
using kanata::macos::timestamped_input_event;

constexpr uint32_t keyboard_page = 0x07;
constexpr uint32_t a_key = 0x04;
constexpr uint32_t b_key = 0x05;
constexpr uint32_t left_shift = 0xe1;

timestamped_input_event event(uint32_t code,
                              uint64_t value,
                              uint64_t timestamp = 100,
                              uint64_t device = 1) {
  return {value, keyboard_page, code, device, timestamp};
}

std::vector<uint32_t> codes(
    const std::vector<timestamped_input_event>& events) {
  std::vector<uint32_t> result;
  for (const auto& input : events) {
    result.push_back(input.code);
  }
  return result;
}

void modifier_press_moves_before_key_from_same_report() {
  std::vector events{event(a_key, 1), event(left_shift, 1)};
  reorder_same_report_events(events);
  assert((codes(events) == std::vector<uint32_t>{left_shift, a_key}));
}

void modifier_release_moves_after_key_from_same_report() {
  std::vector events{event(left_shift, 0), event(a_key, 0)};
  reorder_same_report_events(events);
  assert((codes(events) == std::vector<uint32_t>{a_key, left_shift}));
}

void different_reports_keep_arrival_order() {
  std::vector events{event(a_key, 1, 100), event(left_shift, 1, 101)};
  reorder_same_report_events(events);
  assert((codes(events) == std::vector<uint32_t>{a_key, left_shift}));
}

void different_devices_keep_arrival_order() {
  std::vector events{event(a_key, 1, 100, 1),
                     event(left_shift, 1, 100, 2)};
  reorder_same_report_events(events);
  assert((codes(events) == std::vector<uint32_t>{a_key, left_shift}));
}

void nonmodifier_order_is_stable() {
  std::vector events{event(b_key, 1), event(a_key, 1)};
  reorder_same_report_events(events);
  assert((codes(events) == std::vector<uint32_t>{b_key, a_key}));
}

void mixed_press_and_release_order_is_stable() {
  std::vector events{event(a_key, 1),
                     event(left_shift, 0),
                     event(b_key, 0)};
  reorder_same_report_events(events);
  assert((codes(events) ==
          std::vector<uint32_t>{a_key, b_key, left_shift}));
}

void press_and_release_runs_are_ordered_independently() {
  std::vector events{event(a_key, 1),
                     event(left_shift, 1),
                     event(left_shift, 0),
                     event(a_key, 0)};
  reorder_same_report_events(events);
  assert((codes(events) ==
          std::vector<uint32_t>{left_shift, a_key, a_key, left_shift}));
  assert(events[0].value == 1);
  assert(events[1].value == 1);
  assert(events[2].value == 0);
  assert(events[3].value == 0);
}

} // namespace

int main() {
  modifier_press_moves_before_key_from_same_report();
  modifier_release_moves_after_key_from_same_report();
  different_reports_keep_arrival_order();
  different_devices_keep_arrival_order();
  nonmodifier_order_is_stable();
  mixed_press_and_release_order_is_stable();
  press_and_release_runs_are_ordered_independently();
  std::cout << "input event ordering tests passed\n";
}
