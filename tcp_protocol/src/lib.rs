//! Kanata TCP Protocol
//!
//! This crate defines the JSON message format for communication between
//! TCP clients and the Kanata keyboard remapping daemon.
//!
//! ## Protocol Commands
//!
//! - `Hello` / `HelloOk`: Version and capability detection
//! - `Status` / `StatusInfo`: Server health monitoring
//! - `Reload` with `wait` / `timeout_ms`: Synchronous reload confirmation
//!
//! ## KeyPath Extensions (Fork-only)
//!
//! - `KeyInput` / `KeyOutput`: Live key event streaming for overlay
//! - `HoldActivated`: Tap-hold state notifications
//! - `Ready` / `ConfigError`: Config reload status events

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
        /// Optional correlation ID, echoed from request if provided.
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
    },
    /// Response to `Hello` command with server capabilities.
    HelloOk {
        version: String,
        protocol: u8,
        capabilities: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
    },
    /// Response to `Status` command with engine health information.
    StatusInfo {
        engine_version: String,
        uptime_s: u64,
        ready: bool,
        last_reload: LastReloadInfo,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
    },
    /// Response to Reload commands when `wait: true` was specified.
    ReloadResult {
        ready: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
    },
    /// Broadcast when config reload completes successfully.
    Ready {
        at: String,
    },
    /// Broadcast when config reload fails.
    ConfigError {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        column: Option<u32>,
        at: String,
    },
    // === KeyPath Extensions (Fork-only) ===
    /// Key input event (what the user physically pressed)
    KeyInput {
        key: String,
        action: LiveKeyAction,
        /// Timestamp in milliseconds since Kanata start
        t: u64,
    },
    /// Key output event (what Kanata emits after processing)
    KeyOutput {
        key: String,
        action: LiveKeyAction,
        /// Timestamp in milliseconds since Kanata start
        t: u64,
    },
    /// Sent when a tap-hold key transitions to hold state
    HoldActivated {
        /// Physical key name (e.g., "caps")
        key: String,
        /// Hold action description (e.g., "lctl+lmet+lalt+lsft")
        action: String,
        /// Timestamp in milliseconds since Kanata start
        t: u64,
    },
}

/// Action type for live key events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LiveKeyAction {
    Press,
    Release,
    Repeat,
}

/// Information about the last configuration reload.
#[derive(Debug, Serialize, Deserialize)]
pub struct LastReloadInfo {
    pub ok: bool,
    /// Timestamp as epoch seconds (Unix timestamp).
    pub at: String,
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
    // === Existing commands (unchanged from upstream) ===
    ChangeLayer {
        new: String,
    },
    RequestLayerNames {},
    RequestCurrentLayerInfo {},
    RequestCurrentLayerName {},
    ActOnFakeKey {
        name: String,
        action: FakeKeyActionMessage,
    },
    SetMouse {
        x: u16,
        y: u16,
    },

    // === Reload commands (with wait/timeout support) ===
    /// Reload the current configuration file.
    Reload {
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
    },
    ReloadNext {
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
    },
    ReloadPrev {
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
    },
    ReloadNum {
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
    },
    ReloadFile {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        wait: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
    },

    // === New commands ===
    /// Request server capabilities and version.
    Hello {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
    },
    /// Request engine status information.
    Status {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<u64>,
    },
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
        assert_eq!(
            serde_json::to_string(&ServerResponse::Error {
                msg: "test".to_string()
            })
            .unwrap(),
            r#"{"status":"Error","msg":"test"}"#
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
            request_id: Some(42),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("HelloOk"));
        assert!(json.contains("request_id\":42"));
    }

    #[test]
    fn test_hello_ok_omits_none_request_id() {
        let msg = ServerMessage::HelloOk {
            version: "1.10.0".to_string(),
            protocol: 1,
            capabilities: vec![],
            request_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("request_id"));
    }

    #[test]
    fn test_reload_with_wait() {
        let msg = ClientMessage::Reload {
            wait: Some(true),
            timeout_ms: Some(5000),
            request_id: Some(1),
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
            ClientMessage::Reload {
                wait,
                timeout_ms,
                request_id,
            } => {
                assert!(wait.is_none());
                assert!(timeout_ms.is_none());
                assert!(request_id.is_none());
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
    fn test_status_info() {
        let msg = ServerMessage::StatusInfo {
            engine_version: "1.10.0".to_string(),
            uptime_s: 3600,
            ready: true,
            last_reload: LastReloadInfo {
                ok: true,
                at: "1730619223".to_string(),
            },
            request_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("StatusInfo"));
        assert!(json.contains("uptime_s\":3600"));
    }

    #[test]
    fn test_key_input_event() {
        let msg = ServerMessage::KeyInput {
            key: "a".to_string(),
            action: LiveKeyAction::Press,
            t: 12345,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("KeyInput"));
        assert!(json.contains("\"action\":\"press\""));
    }

    #[test]
    fn test_hold_activated_event() {
        let msg = ServerMessage::HoldActivated {
            key: "caps".to_string(),
            action: "lctl".to_string(),
            t: 12345,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("HoldActivated"));
        assert!(json.contains("\"key\":\"caps\""));
    }
}
