# Request ID Implementation Summary

## ✅ Work Completed

### 1. Protocol Changes (tcp_protocol/src/lib.rs)

**ClientMessage** - ✅ ALL variants have `request_id: Option<u64>`
- Authenticate, ChangeLayer, RequestLayerNames, RequestCurrentLayerInfo
- RequestCurrentLayerName, ActOnFakeKey, SetMouse
- Reload, ReloadNext, ReloadPrev, ReloadNum, ReloadFile
- Hello, Status, Validate, Subscribe

**ServerMessage** - ⚠️ PARTIAL (HelloOk done, others pending)
- ✅ HelloOk - has request_id
- ⏳ StatusInfo - NEEDS request_id
- ⏳ ReloadResult - NEEDS request_id
- ⏳ ValidationResult - NEEDS request_id
- ⏳ Error - NEEDS request_id
- ⏳ ErrorDetail - NEEDS request_id
- ⏳ AuthResult - NEEDS request_id
- ❌ LayerChange, ConfigFileReload, MessagePush, Ready, ConfigError - NO request_id (broadcasts)

### 2. Server Handler Updates (src/tcp_server.rs)

**✅ Completed:**
- Hello handler - extracts and echoes request_id
- Status, Validate, Subscribe, Reload variants - extract request_id

**⏳ Pending:**
- Update ServerMessage constructions to include request_id field

## 🔧 Next Steps to Complete

### Step 1: Finish ServerMessage Protocol Changes

Add `request_id: Option<u64>` to remaining response variants in `tcp_protocol/src/lib.rs`:

```rust
StatusInfo {
    engine_version: String,
    uptime_s: u64,
    ready: bool,
    last_reload: LastReloadInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<u64>,  // ADD THIS
},

ReloadResult {
    ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<u64>,  // ADD THIS
},

ValidationResult {
    #[serde(default)]
    warnings: Vec<ValidationItem>,
    #[serde(default)]
    errors: Vec<ValidationItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<u64>,  // ADD THIS
},

Error {
    msg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<u64>,  // ADD THIS
},
```

### Step 2: Update Server Handler Constructions

In `src/tcp_server.rs`, add `request_id` field to all ServerMessage constructions:

```rust
// Example for StatusInfo
let msg = ServerMessage::StatusInfo {
    engine_version: version,
    uptime_s: uptime.as_secs(),
    ready: is_ready,
    last_reload: last_reload_info,
    request_id,  // ADD THIS - echoes from request
};

// Example for ReloadResult
ServerMessage::ReloadResult {
    ready: is_ready,
    timeout_ms: Some(timeout),
    request_id,  // ADD THIS
}
```

### Step 3: Build and Test

```bash
cd /Users/malpern/local-code/KeyPath/External/kanata
cargo build --release

# Test with nc
echo '{"Hello":{"request_id":42}}' | nc localhost 37001
# Should return:
# {"status":"Ok"}
# {"HelloOk":{"version":"1.10.0","protocol":1,"capabilities":[...],"request_id":42}}
```

### Step 4: Commit to Fork

```bash
git add tcp_protocol/src/lib.rs src/tcp_server.rs
git commit -m "feat: add request_id support to TCP protocol

- Add optional request_id field to all ClientMessage variants
- Add optional request_id field to all ServerMessage response variants
- Server echoes request_id from request to response
- Broadcasts (LayerChange, etc.) never include request_id
- Fully backward compatible (optional on both sides)

This enables reliable response correlation for TCP clients, eliminating
the need for broadcast drain loops.
"
```

### Step 5: Update KeyPath Client

In `/Users/malpern/local-code/KeyPath/Sources/KeyPath/Services/KanataTCPClient.swift`:

1. Generate request_id for each request
2. Match responses by request_id
3. Remove broadcast drain loop
4. Fallback to old behavior for old servers

### Step 6: Deploy and Test

```bash
cd /Users/malpern/local-code/KeyPath
SKIP_NOTARIZE=1 ./build.sh
# Test Hello command
# Test config reload
# Verify no broadcast drain needed
```

## 📊 Benefits

- ✅ **Eliminates broadcast drain loop** - No more 10-attempt limits
- ✅ **Reliable response correlation** - Request ID matching
- ✅ **Backward compatible** - Works with old clients and servers
- ✅ **Enables async clients** - Multiple in-flight requests
- ✅ **Reduces latency** - No waiting for broadcast draining

## 🔍 Testing Checklist

- [ ] Old client + new server (should ignore request_id)
- [ ] New client + old server (should fall back to drain loop)
- [ ] New client + new server (should use request_id matching)
- [ ] Broadcasts never have request_id
- [ ] Response always echoes request_id if provided
- [ ] Multiple concurrent requests work correctly

## 📝 Files Modified

- `External/kanata/tcp_protocol/src/lib.rs` - Protocol definitions
- `External/kanata/src/tcp_server.rs` - Server handlers
- `Sources/KeyPath/Services/KanataTCPClient.swift` - Client implementation (pending)

## ⚠️ Known Issues

- Python script broke enum structure - use manual Edit tool instead
- Need to complete remaining ServerMessage variants
- Server handler constructions need manual updates
