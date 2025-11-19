#!/bin/bash
# Update TCP server handlers to extract and echo request_id

set -e

FILE="src/tcp_server.rs"

echo "Updating $FILE to echo request_id in responses..."

# The strategy: Change destructuring patterns to extract request_id, then include it in responses

# Pattern 1: Change { .. } to { request_id, .. } in match arms
sed -i.bak1 's/ClientMessage::Hello { \.\. }/ClientMessage::Hello { request_id, .. }/g' "$FILE"
sed -i.bak2 's/ClientMessage::Status { \.\. }/ClientMessage::Status { request_id, .. }/g' "$FILE"
sed -i.bak3 's/ClientMessage::Reload { \(.*\) }/ClientMessage::Reload { request_id, \1 }/g' "$FILE"
sed -i.bak4 's/ClientMessage::ReloadNext { \(.*\) }/ClientMessage::ReloadNext { request_id, \1 }/g' "$FILE"
sed -i.bak5 's/ClientMessage::ReloadPrev { \(.*\) }/ClientMessage::ReloadPrev { request_id, \1 }/g' "$FILE"
sed -i.bak6 's/ClientMessage::ReloadNum { \(.*\) }/ClientMessage::ReloadNum { request_id, \1 }/g' "$FILE"
sed-i.bak7 's/ClientMessage::ReloadFile { \(.*\) }/ClientMessage::ReloadFile { request_id, \1 }/g' "$FILE"
sed -i.bak8 's/ClientMessage::Validate { \(.*\) }/ClientMessage::Validate { request_id, \1 }/g' "$FILE"
sed -i.bak9 's/ClientMessage::Subscribe { \(.*\) }/ClientMessage::Subscribe { request_id, \1 }/g' "$FILE"

echo "✅ Updated destructuring patterns to extract request_id"

# Pattern 2: Add request_id field to ServerMessage constructions
# This is more complex - we need to add it as a field

# For HelloOk
sed -i.bak10 's/ServerMessage::HelloOk {$/ServerMessage::HelloOk {\n                                                    request_id,/g' "$FILE"

# For StatusInfo
sed -i.bak11 's/ServerMessage::StatusInfo {$/ServerMessage::StatusInfo {\n                                                    request_id,/g' "$FILE"

# For ReloadResult
sed -i.bak12 's/ServerMessage::ReloadResult {$/ServerMessage::ReloadResult {\n                                                    request_id,/g' "$FILE"

# For ValidationResult
sed -i.bak13 's/ServerMessage::ValidationResult {$/ServerMessage::ValidationResult {\n                                                    request_id,/g' "$FILE"

echo "✅ Updated ServerMessage constructions to include request_id"
echo "Done! Check the file and compile to verify changes."
