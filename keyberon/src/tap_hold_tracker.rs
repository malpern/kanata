//! Tracks tap-hold activation events for external consumers (e.g. TCP broadcast).
//!
//! When the `tap_hold_tracker` feature is enabled, this module stores the
//! coordinate of the most recent hold/tap activation so that higher-level code
//! can relay it over the network.  When the feature is disabled the tracker is
//! a zero-sized no-op — all setters are empty and all getters return `None`.
//!
//! The `config` parameter on the setters accepts a `&WaitingConfig` reference;
//! the `matches!` guard lives inside the method body so that the no-op stub's
//! empty body causes the compiler to eliminate the call entirely.

use crate::layout::KCoord;

/// Why a tap-hold key resolved the way it did.
///
/// Each variant names the specific decision path that determined the outcome.
/// This is intended for debugging and tooling — it carries near-zero cost
/// (one enum write per tap-hold resolution).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapHoldReason {
    // ── Tap reasons ──────────────────────────────────────────────
    /// A different key was pressed recently (`tap-hold-require-prior-idle`),
    /// so the key resolved as tap before entering the waiting state.
    PriorIdle,
    /// The hold-tap key was released before the timeout expired.
    ReleaseBeforeTimeout,
    /// A same-hand key was pressed (opposite-hand custom closure).
    SameHandRoll,
    /// A key from the `tap-hold-keys` / `tap-hold-except` list was pressed.
    CustomTapKeys,
    /// A key from the `tap-hold-release-keys` press-trigger list was pressed.
    CustomReleaseTrigger,

    // ── Hold reasons ─────────────────────────────────────────────
    /// An opposite-hand key was pressed (opposite-hand custom closure).
    OppositeHand,
    /// Any other key was pressed (`HoldOnOtherKeyPress` config).
    OtherKeyPress,
    /// A press-then-release pair was detected in the queue (`PermissiveHold`).
    PermissiveHold,
    /// The tap-hold timeout expired without a release or other trigger.
    Timeout,
    /// The hold-tap key was released after the timeout (`release-after-timeout`).
    ReleaseAfterTimeout,
    /// A custom closure returned Hold without specifying a reason.
    CustomHold,

    // ── Neutral / edge cases ─────────────────────────────────────
    /// A neutral key triggered hold or tap (opposite-hand custom closure).
    NeutralKey,
    /// An unknown-hand key triggered hold or tap (opposite-hand custom closure).
    UnknownHand,
}

impl TapHoldReason {
    /// Stable string identifier for this reason, suitable for logs and TCP messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PriorIdle => "prior-idle",
            Self::ReleaseBeforeTimeout => "release-before-timeout",
            Self::SameHandRoll => "same-hand-roll",
            Self::CustomTapKeys => "custom-tap-keys",
            Self::CustomReleaseTrigger => "custom-release-trigger",
            Self::OppositeHand => "opposite-hand",
            Self::OtherKeyPress => "other-key-press",
            Self::PermissiveHold => "permissive-hold",
            Self::Timeout => "timeout",
            Self::ReleaseAfterTimeout => "release-after-timeout",
            Self::CustomHold => "custom-hold",
            Self::NeutralKey => "neutral-key",
            Self::UnknownHand => "unknown-hand",
        }
    }
}

impl std::fmt::Display for TapHoldReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Information about a tap-hold key that just transitioned to hold state.
#[derive(Debug, Clone, Copy)]
pub struct HoldActivatedInfo {
    /// The key coordinate (row, column).
    pub coord: KCoord,
    /// Why this key resolved as hold.
    pub reason: TapHoldReason,
}

/// Information about a tap-hold key that just triggered its tap action.
#[derive(Debug, Clone, Copy)]
pub struct TapActivatedInfo {
    /// The key coordinate (row, column).
    pub coord: KCoord,
    /// Why this key resolved as tap.
    pub reason: TapHoldReason,
}

#[cfg(feature = "tap_hold_tracker")]
mod inner {
    use super::{HoldActivatedInfo, TapActivatedInfo, TapHoldReason};
    use crate::layout::{KCoord, WaitingConfig};

    /// Records the most recent tap-hold activation event.
    #[derive(Debug, Default)]
    pub struct TapHoldTracker {
        hold_activated: Option<HoldActivatedInfo>,
        tap_activated: Option<TapActivatedInfo>,
    }

    impl TapHoldTracker {
        pub(crate) fn set_hold_activated<'a, T: std::fmt::Debug>(
            &mut self,
            coord: KCoord,
            config: &WaitingConfig<'a, T>,
            reason: TapHoldReason,
        ) {
            if matches!(config, WaitingConfig::HoldTap(..)) {
                self.hold_activated = Some(HoldActivatedInfo { coord, reason });
            }
        }

        pub(crate) fn set_tap_activated<'a, T: std::fmt::Debug>(
            &mut self,
            coord: KCoord,
            config: &WaitingConfig<'a, T>,
            reason: TapHoldReason,
        ) {
            if matches!(config, WaitingConfig::HoldTap(..)) {
                self.tap_activated = Some(TapActivatedInfo { coord, reason });
            }
        }

        pub fn take_hold_activated(&mut self) -> Option<HoldActivatedInfo> {
            self.hold_activated.take()
        }

        pub fn take_tap_activated(&mut self) -> Option<TapActivatedInfo> {
            self.tap_activated.take()
        }
    }
}

#[cfg(not(feature = "tap_hold_tracker"))]
mod inner {
    use super::{HoldActivatedInfo, TapActivatedInfo, TapHoldReason};
    use crate::layout::{KCoord, WaitingConfig};

    /// Zero-sized no-op tracker when the feature is disabled.
    #[derive(Debug, Default)]
    pub struct TapHoldTracker;

    impl TapHoldTracker {
        #[inline(always)]
        pub(crate) fn set_hold_activated<'a, T: std::fmt::Debug>(
            &mut self,
            _coord: KCoord,
            _config: &WaitingConfig<'a, T>,
            _reason: TapHoldReason,
        ) {
        }

        #[inline(always)]
        pub(crate) fn set_tap_activated<'a, T: std::fmt::Debug>(
            &mut self,
            _coord: KCoord,
            _config: &WaitingConfig<'a, T>,
            _reason: TapHoldReason,
        ) {
        }

        #[inline(always)]
        pub fn take_hold_activated(&mut self) -> Option<HoldActivatedInfo> {
            None
        }

        #[inline(always)]
        pub fn take_tap_activated(&mut self) -> Option<TapActivatedInfo> {
            None
        }
    }
}

// Re-export shared types and the cfg-selected tracker.
pub use inner::TapHoldTracker;
