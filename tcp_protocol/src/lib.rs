//! Kanata TCP Protocol
//!
//! This crate defines the JSON message format for communication between
//! TCP clients and the Kanata keyboard remapping daemon.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Messages sent from the server to connected clients.
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    LayerChange {
        new: String,
    },
    LayerNames {
        names: Vec<String>,
    },
    FakeKeyNames {
        names: Vec<String>,
    },
    CurrentLayerInfo {
        name: String,
        cfg_text: String,
    },
    ConfigFileReload {
        new: String,
    },
    CurrentLayerName {
        name: String,
    },
    MessagePush {
        message: serde_json::Value,
    },
    Error {
        msg: String,
    },
    /// Response to `Hello` command with server capabilities.
    /// Introduced in protocol v1.11.
    HelloOk {
        version: String,
        protocol: u8,
        capabilities: Vec<String>,
    },
    /// Response to Reload commands when `wait: true` was specified.
    /// Introduced in protocol v1.11.
    ReloadResult {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    /// Sent when a tap-hold key transitions to hold state.
    /// The `key` field is the physical key name (e.g., `"caps"`, `"a"`).
    /// The `reason` field explains why this decision was made.
    HoldActivated {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// Sent when a tap-hold key triggers its tap action.
    /// The `key` field is the physical key name (e.g., `"caps"`, `"a"`).
    /// The `reason` field explains why this decision was made.
    TapActivated {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// Sent on every physical key press/release for live overlay visualization.
    /// The `key` field is the physical key name (lowercase, e.g., `"space"`, `"a"`).
    KeyInput {
        key: String,
        action: LiveKeyAction,
        /// Milliseconds since kanata started (monotonic timestamp).
        t: u64,
    },
    /// Sent when a chord (multi-key combo) resolves to an action.
    ChordResolved {
        keys: String,
        action: String,
        t: u64,
    },
    /// Sent when a tap-dance resolves to an action.
    TapDanceResolved {
        key: String,
        tap_count: u16,
        action: String,
        t: u64,
    },
    /// Authoritative report of whether kanata has physically grabbed (seized)
    /// the input keyboard device(s).
    ///
    /// This is ground truth straight from the OS grab layer — it is NOT
    /// inferred from key-event flow, so it is immune to the ambiguity of
    /// "no keys seen" (idle user vs. failed grab vs. synthetic/VNC input that
    /// bypasses the physical seize).
    ///
    /// Emitted on startup once the grab attempt resolves, and again on any
    /// change afterward (devices released for recovery/lock, re-seized via the
    /// recovery path, or devices added/removed).
    ///
    /// - `active`: true if at least one physical device is currently seized.
    /// - `devices`: names of the currently seized devices. Always present;
    ///   empty when `active` is false.
    /// - `reason`: optional human-readable explanation, typically populated on
    ///   failure (e.g. another process holds an exclusive grab, not running as
    ///   root, driver not approved).
    InputGrab {
        active: bool,
        devices: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

/// Action type for KeyInput events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiveKeyAction {
    Press,
    Release,
    Repeat,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "status")]
pub enum ServerResponse {
    Ok,
    Error { msg: String },
}

impl ServerResponse {
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut msg = serde_json::to_vec(self).expect("ServerResponse should serialize");
        msg.push(b'\n');
        msg
    }
}

impl ServerMessage {
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut msg = serde_json::to_vec(self).expect("ServerMessage should serialize");
        msg.push(b'\n');
        msg
    }
}

/// Messages sent from clients to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    ChangeLayer {
        new: String,
    },
    RequestLayerNames {},
    RequestFakeKeyNames {},
    RequestCurrentLayerInfo {},
    RequestCurrentLayerName {},
    /// Request the current authoritative input-grab status on demand.
    ///
    /// The server normally emits `ServerMessage::InputGrab` only on grab-state
    /// transitions (startup resolution and subsequent changes). A client that
    /// connects after startup misses that one-shot signal, so it can send this
    /// to fetch the last known status. The response is a `ServerMessage::InputGrab`
    /// identical in shape to the transition-emitted one. If the grab has not yet
    /// resolved (nothing cached), the server sends no response (the client falls
    /// back to its own detection).
    RequestInputGrab {},
    ActOnFakeKey {
        name: String,
        action: FakeKeyActionMessage,
    },
    SetMouse {
        x: u16,
        y: u16,
    },

    /// Reload the current configuration file.
    Reload {
        /// If true, block until reload completes or times out.
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        /// Maximum time to wait for reload (milliseconds). Default: 5000.
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadNext {
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadPrev {
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadNum {
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    ReloadFile {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },

    /// Request server capabilities and version.
    /// Introduced in protocol v1.11.
    Hello {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum FakeKeyActionMessage {
    Press,
    Release,
    Tap,
    Toggle,
}

impl FromStr for ClientMessage {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_response_json_format() {
        assert_eq!(
            serde_json::to_string(&ServerResponse::Ok).unwrap(),
            r#"{"status":"Ok"}"#
        );
    }

    #[test]
    fn test_as_bytes_includes_newline() {
        let response = ServerResponse::Ok;
        assert!(response.as_bytes().ends_with(b"\n"));
    }

    #[test]
    fn test_hello_ok_json_format() {
        let msg = ServerMessage::HelloOk {
            version: "1.10.0".to_string(),
            protocol: 1,
            capabilities: vec!["reload".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("HelloOk"));
        assert!(json.contains("\"version\":\"1.10.0\""));
    }

    #[test]
    fn test_reload_with_wait() {
        let msg = ClientMessage::Reload {
            wait: Some(true),
            timeout_ms: Some(5000),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("wait\":true"));
        assert!(json.contains("timeout_ms\":5000"));
    }

    #[test]
    fn test_reload_minimal() {
        // Backward compatible: no optional fields
        let json = r#"{"Reload":{}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Reload { wait, timeout_ms } => {
                assert!(wait.is_none());
                assert!(timeout_ms.is_none());
            }
            _ => panic!("Expected Reload"),
        }
    }

    #[test]
    fn test_existing_commands_unchanged() {
        // Verify existing commands still parse without any new fields
        let json = r#"{"ChangeLayer":{"new":"nav"}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::ChangeLayer { new } if new == "nav"));

        let json = r#"{"RequestLayerNames":{}}"#;
        let _msg: ClientMessage = serde_json::from_str(json).unwrap();

        let json = r#"{"ActOnFakeKey":{"name":"test","action":"Tap"}}"#;
        let _msg: ClientMessage = serde_json::from_str(json).unwrap();
    }

    #[test]
    fn test_request_input_grab_round_trip() {
        // Wire format the Swift client sends after Hello.
        let json = r#"{"RequestInputGrab":{}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::RequestInputGrab {}));

        // Serializes back to the canonical empty-struct form.
        let serialized = serde_json::to_string(&ClientMessage::RequestInputGrab {}).unwrap();
        assert_eq!(serialized, json);
    }

    #[test]
    fn test_request_fake_key_names() {
        let json = r#"{"RequestFakeKeyNames":{}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::RequestFakeKeyNames {}));
    }

    #[test]
    fn test_fake_key_names_response() {
        let msg = ServerMessage::FakeKeyNames {
            names: vec!["email-sig".to_string(), "nav-mode".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"FakeKeyNames":{"names":["email-sig","nav-mode"]}}"#
        );

        // Round-trip
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ServerMessage::FakeKeyNames { names } => {
                assert_eq!(names, vec!["email-sig", "nav-mode"]);
            }
            _ => panic!("Expected FakeKeyNames"),
        }
    }

    #[test]
    fn test_fake_key_names_empty() {
        let msg = ServerMessage::FakeKeyNames { names: vec![] };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"FakeKeyNames":{"names":[]}}"#);
    }

    #[test]
    fn test_hold_activated_json_format() {
        let msg = ServerMessage::HoldActivated {
            key: "caps".to_string(),
            reason: Some("opposite-hand".into()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"HoldActivated":{"key":"caps","reason":"opposite-hand"}}"#
        );
    }

    #[test]
    fn test_hold_activated_no_reason() {
        // Backward compatible: reason is optional
        let msg = ServerMessage::HoldActivated {
            key: "caps".to_string(),
            reason: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"HoldActivated":{"key":"caps"}}"#);
    }

    #[test]
    fn test_tap_activated_json_format() {
        let msg = ServerMessage::TapActivated {
            key: "a".to_string(),
            reason: Some("prior-idle".into()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"TapActivated":{"key":"a","reason":"prior-idle"}}"#
        );
    }

    #[test]
    fn test_tap_activated_no_reason() {
        let msg = ServerMessage::TapActivated {
            key: "a".to_string(),
            reason: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"TapActivated":{"key":"a"}}"#);
    }

    #[test]
    fn test_key_input_json_format() {
        let msg = ServerMessage::KeyInput {
            key: "space".to_string(),
            action: LiveKeyAction::Press,
            t: 12345,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"KeyInput":{"key":"space","action":"Press","t":12345}}"#
        );

        // Round-trip
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ServerMessage::KeyInput { key, action, t } => {
                assert_eq!(key, "space");
                assert_eq!(action, LiveKeyAction::Press);
                assert_eq!(t, 12345);
            }
            _ => panic!("Expected KeyInput"),
        }
    }

    #[test]
    fn test_chord_resolved_json_format() {
        let msg = ServerMessage::ChordResolved {
            keys: "s+d".to_string(),
            action: "esc".to_string(),
            t: 12345,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"ChordResolved":{"keys":"s+d","action":"esc","t":12345}}"#
        );
    }

    #[test]
    fn test_input_grab_active_json_format() {
        let msg = ServerMessage::InputGrab {
            active: true,
            devices: vec![
                "Apple Internal Keyboard / Trackpad".to_string(),
                "HHKB-Hybrid".to_string(),
            ],
            reason: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"InputGrab":{"active":true,"devices":["Apple Internal Keyboard / Trackpad","HHKB-Hybrid"]}}"#
        );

        // Round-trip
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ServerMessage::InputGrab {
                active,
                devices,
                reason,
            } => {
                assert!(active);
                assert_eq!(
                    devices,
                    vec!["Apple Internal Keyboard / Trackpad", "HHKB-Hybrid"]
                );
                assert!(reason.is_none());
            }
            _ => panic!("Expected InputGrab"),
        }
    }

    #[test]
    fn test_input_grab_failure_json_format() {
        let msg = ServerMessage::InputGrab {
            active: false,
            devices: vec![],
            reason: Some("another process has exclusive grab".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"InputGrab":{"active":false,"devices":[],"reason":"another process has exclusive grab"}}"#
        );

        // Round-trip
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ServerMessage::InputGrab {
                active,
                devices,
                reason,
            } => {
                assert!(!active);
                assert!(devices.is_empty());
                assert_eq!(
                    reason.as_deref(),
                    Some("another process has exclusive grab")
                );
            }
            _ => panic!("Expected InputGrab"),
        }
    }

    #[test]
    fn test_input_grab_no_reason_omits_field() {
        // Stable schema: devices always present, reason omitted when None.
        let msg = ServerMessage::InputGrab {
            active: false,
            devices: vec![],
            reason: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"InputGrab":{"active":false,"devices":[]}}"#);
    }

    #[test]
    fn test_tap_dance_resolved_json_format() {
        let msg = ServerMessage::TapDanceResolved {
            key: "q".to_string(),
            tap_count: 2,
            action: "alt+tab".to_string(),
            t: 12345,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(
            json,
            r#"{"TapDanceResolved":{"key":"q","tap_count":2,"action":"alt+tab","t":12345}}"#
        );
    }
}
