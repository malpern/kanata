#!/bin/bash
# Add request_id to ServerMessage response variants (not broadcasts)
# Broadcasts: LayerChange, ConfigFileReload, MessagePush, AuthRequired, SessionExpired, Ready, ConfigError
# Responses: Error, AuthResult, HelloOk, StatusInfo, ReloadResult, ValidationResult, ErrorDetail, LayerNames, CurrentLayerInfo, CurrentLayerName

set -e

FILE="tcp_protocol/src/lib.rs"

echo "Adding request_id to ServerMessage response variants..."

# Use a more targeted sed approach - add request_id as the last field before closing brace

# HelloOk
sed -i.bak '/HelloOk {/,/}/s/capabilities: Vec<String>,$/capabilities: Vec<String>,\
        #[serde(skip_serializing_if = "Option::is_none")]\
        request_id: Option<u64>,/' "$FILE"

# StatusInfo
sed -i.bak2 '/StatusInfo {/,/}/s/last_reload: LastReloadInfo,$/last_reload: LastReloadInfo,\
        #[serde(skip_serializing_if = "Option::is_none")]\
        request_id: Option<u64>,/' "$FILE"

# ReloadResult
sed -i.bak3 '/ReloadResult {/,/}/s/timeout_ms: Option<u64>,$/timeout_ms: Option<u64>,\
        #[serde(skip_serializing_if = "Option::is_none")]\
        request_id: Option<u64>,/' "$FILE"

# ValidationResult
sed -i.bak4 '/ValidationResult {/,/}/s/errors: Vec<ValidationItem>,$/errors: Vec<ValidationItem>,\
        #[serde(skip_serializing_if = "Option::is_none")]\
        request_id: Option<u64>,/' "$FILE"

# Error
sed -i.bak5 '/^    Error {$/,/^    },$/s/msg: String,$/msg: String,\
        #[serde(skip_serializing_if = "Option::is_none")]\
        request_id: Option<u64>,/' "$FILE"

# ErrorDetail
sed -i.bak6 '/ErrorDetail {/,/}/s/column: Option<u32>,$/column: Option<u32>,\
        #[serde(skip_serializing_if = "Option::is_none")]\
        request_id: Option<u64>,/' "$FILE"

# AuthResult
sed -i.bak7 '/AuthResult {/,/}/s/expires_in_seconds: Option<u64>,$/expires_in_seconds: Option<u64>,\
        #[serde(skip_serializing_if = "Option::is_none")]\
        request_id: Option<u64>,/' "$FILE"

echo "✅ Added request_id to ServerMessage response variants"
echo "Done!"
