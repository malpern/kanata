use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    LayerChange { new: String },
    LayerNames { names: Vec<String> },
    CurrentLayerInfo { name: String, cfg_text: String },
    ConfigFileReload { new: String },
    CurrentLayerName { name: String },
    MessagePush { message: serde_json::Value },
    Error { msg: String },
    // UDP Authentication messages
    AuthResult { 
        success: bool,
        session_id: Option<String>,
        expires_in_seconds: Option<u64>,
    },
    AuthRequired,
    SessionExpired,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    // UDP Authentication message
    Authenticate { 
        token: String,
        client_name: Option<String>,
    },
    // Existing messages with optional session_id for UDP auth
    ChangeLayer {
        new: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    RequestLayerNames {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    RequestCurrentLayerInfo {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    RequestCurrentLayerName {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    ActOnFakeKey {
        name: String,
        action: FakeKeyActionMessage,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    SetMouse {
        x: u16,
        y: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    Reload {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    ReloadNext {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    ReloadPrev {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    ReloadNum {
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    ReloadFile {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
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
        // Test that our API contract matches expected JSON structure
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
        // Test our specific logic that adds newline termination
        let response = ServerResponse::Ok;
        let bytes = response.as_bytes();
        assert!(bytes.ends_with(b"\n"), "Response should end with newline");

        let error_response = ServerResponse::Error {
            msg: "test".to_string(),
        };
        let error_bytes = error_response.as_bytes();
        assert!(
            error_bytes.ends_with(b"\n"),
            "Error response should end with newline"
        );
    }
}
