use crate::Kanata;
use crate::oskbd::*;

#[cfg(feature = "tcp_server")]
use kanata_tcp_protocol::*;
use parking_lot::Mutex;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc::SyncSender as Sender;

#[cfg(feature = "tcp_server")]
type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;
#[cfg(feature = "tcp_server")]
use kanata_parser::cfg::SimpleSExpr;
#[cfg(feature = "tcp_server")]
use std::io::Write;
#[cfg(feature = "tcp_server")]
use std::io;
#[cfg(feature = "tcp_server")]
use std::net::{TcpListener, TcpStream};

#[cfg(feature = "tcp_server")]
use std::time::Instant;

// Connection limits to prevent resource exhaustion and keyboard freeze
#[cfg(feature = "tcp_server")]
const MAX_CONCURRENT_CONNECTIONS: usize = 10;
#[cfg(feature = "tcp_server")]
const STALE_CONNECTION_TIMEOUT_SECS: u64 = 300; // 5 minutes

#[cfg(feature = "tcp_server")]
pub struct ClientState {
    pub stream: TcpStream,
    pub last_activity: Instant,
}

#[cfg(feature = "tcp_server")]
impl ClientState {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            last_activity: Instant::now(),
        }
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Check if connection is stale
    pub fn is_stale(&self) -> bool {
        self.last_activity.elapsed().as_secs() > STALE_CONNECTION_TIMEOUT_SECS
    }
}

#[cfg(feature = "tcp_server")]
pub type Connections = Arc<Mutex<HashMap<String, ClientState>>>;

#[cfg(not(feature = "tcp_server"))]
pub type Connections = ();

#[cfg(feature = "tcp_server")]
use kanata_parser::custom_action::FakeKeyAction;

#[cfg(feature = "tcp_server")]
fn send_response(
    stream: &mut TcpStream,
    response: ServerResponse,
    connections: &Connections,
    addr: &str,
) -> bool {
    if let Err(write_err) = stream.write_all(&response.as_bytes()) {
        log::error!("stream write error: {write_err}");
        connections.lock().remove(addr);
        return false;
    }
    true
}

// Best-effort write that treats BrokenPipe as a clean disconnect and always
// cleans up the connection entry.
#[cfg(feature = "tcp_server")]
fn write_with_disconnect_handling(
    stream: &mut TcpStream,
    bytes: &[u8],
    addr: &str,
    connections: &Connections,
    context: &str,
) -> bool {
    match stream.write_all(bytes) {
        Ok(_) => true,
        Err(e) => {
            if e.kind() == io::ErrorKind::BrokenPipe {
                log::warn!(
                    "{context}: client {addr} disconnected before response was fully written (broken pipe)"
                );
            } else {
                log::error!("{context}: {e}");
            }
            connections.lock().remove(addr);
            false
        }
    }
}

// RAII guard to ensure we always remove the connection entry when a handler exits.
#[cfg(feature = "tcp_server")]
struct ConnectionCleanup {
    addr: String,
    connections: Connections,
}

#[cfg(feature = "tcp_server")]
impl ConnectionCleanup {
    fn new(addr: String, connections: Connections) -> Self {
        Self { addr, connections }
    }
}

#[cfg(feature = "tcp_server")]
impl Drop for ConnectionCleanup {
    fn drop(&mut self) {
        self.connections.lock().remove(&self.addr);
    }
}


#[cfg(feature = "tcp_server")]
fn to_action(val: FakeKeyActionMessage) -> FakeKeyAction {
    match val {
        FakeKeyActionMessage::Press => FakeKeyAction::Press,
        FakeKeyActionMessage::Release => FakeKeyAction::Release,
        FakeKeyActionMessage::Tap => FakeKeyAction::Tap,
        FakeKeyActionMessage::Toggle => FakeKeyAction::Toggle,
    }
}

#[cfg(feature = "tcp_server")]
pub struct TcpServer {
    pub address: SocketAddr,
    pub connections: Connections,
    pub wakeup_channel: Sender<KeyEvent>,
}

#[cfg(not(feature = "tcp_server"))]
pub struct TcpServer {
    pub connections: Connections,
}

impl TcpServer {
    #[cfg(feature = "tcp_server")]
    pub fn new(address: SocketAddr, wakeup_channel: Sender<KeyEvent>) -> Self {
        Self {
            address,
            connections: Arc::new(Mutex::new(HashMap::default())),
            wakeup_channel,
        }
    }

    #[cfg(not(feature = "tcp_server"))]
    pub fn new(_address: SocketAddr, _wakeup_channel: Sender<KeyEvent>) -> Self {
        Self { connections: () }
    }

    #[cfg(feature = "tcp_server")]
    pub fn start(&mut self, kanata: Arc<Mutex<Kanata>>) {
        use kanata_parser::cfg::FAKE_KEY_ROW;

        use crate::kanata::handle_fakekey_action;

        let listener = TcpListener::bind(self.address).expect("TCP server starts");

        let connections = self.connections.clone();
        let wakeup_channel = self.wakeup_channel.clone();

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        if let Err(e) = stream.set_nodelay(true) {
                            log::warn!("failed to set nodelay: {e:?}");
                        }
                        if let Err(e) =
                            stream.set_read_timeout(Some(std::time::Duration::from_secs(120)))
                        {
                            log::warn!("failed to set read timeout: {e:?}");
                        }
                        if let Err(e) =
                            stream.set_write_timeout(Some(std::time::Duration::from_secs(10)))
                        {
                            log::warn!("failed to set write timeout: {e:?}");
                        }

                        {
                            let k = kanata.lock();
                            log::info!(
                                "new client connection, sending initial LayerChange event to inform them of current layer"
                            );
                            if let Err(e) = stream.write(
                                &ServerMessage::LayerChange {
                                    new: k.layer_info[k.layout.b().current_layer()].name.clone(),
                                }
                                .as_bytes(),
                            ) {
                                log::warn!("failed to write to stream, dropping it: {e:?}");
                                continue;
                            }
                        }

                        let addr = match stream.peer_addr() {
                            Ok(addr) => addr.to_string(),
                            Err(e) => {
                                log::warn!("failed to get peer address, using fallback: {e:?}");
                                format!("unknown_{}", std::ptr::addr_of!(stream) as usize)
                            }
                        };

                        // Clean up stale connections before accepting new ones
                        {
                            let mut conns = connections.lock();
                            let stale: Vec<String> = conns
                                .iter()
                                .filter(|(_, state)| state.is_stale())
                                .map(|(addr, _)| addr.clone())
                                .collect();

                            for stale_addr in stale {
                                log::info!(
                                    "removing stale connection {stale_addr} (no activity for {STALE_CONNECTION_TIMEOUT_SECS}s)"
                                );
                                conns.remove(&stale_addr);
                            }
                        }

                        // Check connection limit to prevent resource exhaustion
                        {
                            let conns = connections.lock();
                            if conns.len() >= MAX_CONCURRENT_CONNECTIONS {
                                log::warn!(
                                    "connection limit reached ({MAX_CONCURRENT_CONNECTIONS}), rejecting connection from {addr}"
                                );
                                log::warn!(
                                    "this prevents file descriptor exhaustion and keyboard freeze"
                                );
                                drop(stream);
                                continue;
                            }
                        }

                        // Clone stream for connections map - handle errors gracefully
                        let stream_clone_for_map = match stream.try_clone() {
                            Ok(s) => s,
                            Err(e) => {
                                log::error!("failed to clone stream for connections map: {e:?}");
                                log::error!("dropping connection {addr} due to clone failure");
                                drop(stream);
                                continue;
                            }
                        };

                        // Clone stream for reader - handle errors gracefully
                        let stream_clone_for_reader = match stream.try_clone() {
                            Ok(s) => s,
                            Err(e) => {
                                log::error!("failed to clone stream for reader: {e:?}");
                                log::error!("dropping connection {addr} due to clone failure");
                                drop(stream_clone_for_map);
                                drop(stream);
                                continue;
                            }
                        };

                        connections
                            .lock()
                            .insert(addr.clone(), ClientState::new(stream_clone_for_map));
                        let reader = serde_json::Deserializer::from_reader(stream_clone_for_reader)
                            .into_iter::<ClientMessage>();

                        log::info!("listening for incoming messages {addr}");

                        let connections = connections.clone();
                        let kanata = kanata.clone();
                        let wakeup_channel = wakeup_channel.clone();
                        std::thread::spawn(move || {
                            let _cleanup_guard =
                                ConnectionCleanup::new(addr.clone(), connections.clone());
                            for v in reader {
                                match v {
                                    Ok(event) => {
                                        // Update activity timestamp for this connection
                                        if let Some(state) = connections.lock().get_mut(&addr) {
                                            state.update_activity();
                                        }

                                        log::debug!("tcp server received command: {:?}", event);
                                        match event {
                                            ClientMessage::ChangeLayer { new, .. } => {
                                                kanata.lock().change_layer(new);
                                            }
                                            ClientMessage::RequestLayerNames { .. } => {
                                                let msg = ServerMessage::LayerNames {
                                                    names: kanata
                                                        .lock()
                                                        .layer_info
                                                        .iter()
                                                        .map(|info| info.name.clone())
                                                        .collect::<Vec<_>>(),
                                                };
                                                let _ = write_with_disconnect_handling(
                                                    &mut stream,
                                                    &msg.as_bytes(),
                                                    &addr,
                                                    &connections,
                                                    "server could not send response",
                                                );
                                            }
                                            ClientMessage::ActOnFakeKey {
                                                name,
                                                action,
                                            } => {
                                                let mut k = kanata.lock();
                                                let index = match k.virtual_keys.get(&name) {
                                                    Some(index) => Some(*index as u16),
                                                    None => {
                                                        if let Err(e) = stream.write_all(
                                                            &ServerMessage::Error {
                                                                msg: format!(
                                                                "unknown virtual/fake key: {name}"
                                                            ),
                                                                request_id: None,
                                                            }
                                                            .as_bytes(),
                                                        ) {
                                                            log::error!("stream write error: {e}");
                                                            connections.lock().remove(&addr);
                                                            break;
                                                        }
                                                        continue;
                                                    }
                                                };
                                                if let Some(index) = index {
                                                    log::info!(
                                                        "tcp server fake-key action: {name},{action:?}"
                                                    );
                                                    handle_fakekey_action(
                                                        to_action(action),
                                                        k.layout.bm(),
                                                        FAKE_KEY_ROW,
                                                        index,
                                                    );
                                                }
                                                drop(k);
                                            }
                                            ClientMessage::SetMouse { x, y, .. } => {
                                                log::info!(
                                                    "tcp server SetMouse action: x {x} y {y}"
                                                );
                                                match kanata.lock().kbd_out.set_mouse(x, y) {
                                                    Ok(_) => {
                                                        log::info!(
                                                            "sucessfully did set mouse position to: x {x} y {y}"
                                                        );
                                                        // Optionally send a success message to the
                                                        // client
                                                    }
                                                    Err(e) => {
                                                        log::error!(
                                                            "Failed to set mouse position: {}",
                                                            e
                                                        );
                                                        // Implement any error handling logic here,
                                                        // such as sending an error response to
                                                        // the client
                                                    }
                                                }
                                            }
                                            ClientMessage::RequestCurrentLayerInfo { .. } => {
                                                let mut k = kanata.lock();
                                                let cur_layer = k.layout.bm().current_layer();
                                                let msg = ServerMessage::CurrentLayerInfo {
                                                    name: k.layer_info[cur_layer].name.clone(),
                                                    cfg_text: k.layer_info[cur_layer]
                                                        .cfg_text
                                                        .clone(),
                                                };
                                                drop(k);
                                                let _ = write_with_disconnect_handling(
                                                    &mut stream,
                                                    &msg.as_bytes(),
                                                    &addr,
                                                    &connections,
                                                    "Error writing response to RequestCurrentLayerInfo",
                                                );
                                            }
                                            ClientMessage::RequestCurrentLayerName { .. } => {
                                                let mut k = kanata.lock();
                                                let cur_layer = k.layout.bm().current_layer();
                                                let msg = ServerMessage::CurrentLayerName {
                                                    name: k.layer_info[cur_layer].name.clone(),
                                                };
                                                drop(k);
                                                let _ = write_with_disconnect_handling(
                                                    &mut stream,
                                                    &msg.as_bytes(),
                                                    &addr,
                                                    &connections,
                                                    "Error writing response to RequestCurrentLayerName",
                                                );
                                            }
                                            ClientMessage::Hello { request_id, .. } => {
                                                let version = env!("CARGO_PKG_VERSION").to_string();
                                                let capabilities = vec![
                                                    "reload".to_string(),
                                                    "status".to_string(),
                                                    "ready".to_string(),
                                                    // Overlay/telemetry capabilities
                                                    "hold_activated".to_string(),
                                                    "tap_activated".to_string(),
                                                    "oneshot_activated".to_string(),
                                                    "chord_resolved".to_string(),
                                                    "tap_dance_resolved".to_string(),
                                                    "key_input".to_string(),
                                                ];
                                                let msg = ServerMessage::HelloOk {
                                                    version,
                                                    protocol: 1,
                                                    capabilities,
                                                    request_id,
                                                };
                                                // Send status response first
                                                if !send_response(
                                                    &mut stream,
                                                    ServerResponse::Ok,
                                                    &connections,
                                                    &addr,
                                                ) {
                                                    break;
                                                }
                                                // Send HelloOk details on second line
                                                if write_with_disconnect_handling(
                                                    &mut stream,
                                                    &msg.as_bytes(),
                                                    &addr,
                                                    &connections,
                                                    "Error writing HelloOk response",
                                                ) {
                                                    // Flush to ensure immediate delivery
                                                    let _ = stream.flush();
                                                }
                                            }
                                            ClientMessage::Status { request_id, .. } => {
                                                let k = kanata.lock();
                                                let engine_version =
                                                    env!("CARGO_PKG_VERSION").to_string();
                                                let uptime_s = k.get_uptime_s();
                                                let ready = k.is_ready();
                                                let last_reload = k.get_last_reload_info();
                                                drop(k);

                                                let msg = ServerMessage::StatusInfo {
                                                    request_id,
                                                    engine_version,
                                                    uptime_s,
                                                    ready,
                                                    last_reload,
                                                };
                                                // Send status response first
                                                if !send_response(
                                                    &mut stream,
                                                    ServerResponse::Ok,
                                                    &connections,
                                                    &addr,
                                                ) {
                                                    break;
                                                }
                                                // Send StatusInfo details on second line
                                                if write_with_disconnect_handling(
                                                    &mut stream,
                                                    &msg.as_bytes(),
                                                    &addr,
                                                    &connections,
                                                    "Error writing StatusInfo response",
                                                ) {
                                                    // Flush to ensure immediate delivery
                                                    let _ = stream.flush();
                                                }
                                            }

                                            // Handle reload commands with unified response protocol
                                            ref reload_cmd @ (ClientMessage::Reload { .. }
                                            | ClientMessage::ReloadNext {
                                                ..
                                            }
                                            | ClientMessage::ReloadPrev {
                                                ..
                                            }
                                            | ClientMessage::ReloadNum {
                                                ..
                                            }
                                            | ClientMessage::ReloadFile {
                                                ..
                                            }) => {
                                                // Extract request_id from the command
                                                let request_id = match reload_cmd {
                                                    ClientMessage::Reload {
                                                        request_id, ..
                                                    } => *request_id,
                                                    ClientMessage::ReloadNext {
                                                        request_id,
                                                        ..
                                                    } => *request_id,
                                                    ClientMessage::ReloadPrev {
                                                        request_id,
                                                        ..
                                                    } => *request_id,
                                                    ClientMessage::ReloadNum {
                                                        request_id, ..
                                                    } => *request_id,
                                                    ClientMessage::ReloadFile {
                                                        request_id,
                                                        ..
                                                    } => *request_id,
                                                    _ => None,
                                                };

                                                // Extract wait and timeout_ms from the command
                                                let (wait_flag, timeout) = match reload_cmd {
                                                    ClientMessage::Reload {
                                                        wait,
                                                        timeout_ms,
                                                        ..
                                                    } => (*wait, *timeout_ms),
                                                    ClientMessage::ReloadNext {
                                                        wait,
                                                        timeout_ms,
                                                        ..
                                                    } => (*wait, *timeout_ms),
                                                    ClientMessage::ReloadPrev {
                                                        wait,
                                                        timeout_ms,
                                                        ..
                                                    } => (*wait, *timeout_ms),
                                                    ClientMessage::ReloadNum {
                                                        wait,
                                                        timeout_ms,
                                                        ..
                                                    } => (*wait, *timeout_ms),
                                                    ClientMessage::ReloadFile {
                                                        wait,
                                                        timeout_ms,
                                                        ..
                                                    } => (*wait, *timeout_ms),
                                                    _ => (None, None),
                                                };

                                                // Log specific action type
                                                let reload_start_time = std::time::Instant::now();
                                                match &reload_cmd {
                                                    ClientMessage::Reload {
                                                        request_id, ..
                                                    } => {
                                                        log::info!(
                                                            "tcp server Reload action (request_id={:?}, wait={:?}, timeout_ms={:?})",
                                                            request_id,
                                                            wait_flag,
                                                            timeout
                                                        )
                                                    }
                                                    ClientMessage::ReloadNext {
                                                        request_id,
                                                        ..
                                                    } => {
                                                        log::info!(
                                                            "tcp server ReloadNext action (request_id={:?}, wait={:?}, timeout_ms={:?})",
                                                            request_id,
                                                            wait_flag,
                                                            timeout
                                                        )
                                                    }
                                                    ClientMessage::ReloadPrev {
                                                        request_id,
                                                        ..
                                                    } => {
                                                        log::info!(
                                                            "tcp server ReloadPrev action (request_id={:?}, wait={:?}, timeout_ms={:?})",
                                                            request_id,
                                                            wait_flag,
                                                            timeout
                                                        )
                                                    }
                                                    ClientMessage::ReloadNum {
                                                        index,
                                                        request_id,
                                                        ..
                                                    } => {
                                                        log::info!(
                                                            "tcp server ReloadNum action: index {index} (request_id={:?}, wait={:?}, timeout_ms={:?})",
                                                            request_id,
                                                            wait_flag,
                                                            timeout
                                                        )
                                                    }
                                                    ClientMessage::ReloadFile {
                                                        path,
                                                        request_id,
                                                        ..
                                                    } => {
                                                        log::info!(
                                                            "tcp server ReloadFile action: path {path} (request_id={:?}, wait={:?}, timeout_ms={:?})",
                                                            request_id,
                                                            wait_flag,
                                                            timeout
                                                        )
                                                    }
                                                    _ => unreachable!(),
                                                }

                                                log::debug!(
                                                    "Reload: calling handle_client_command"
                                                );
                                                let response = match kanata
                                                    .lock()
                                                    .handle_client_command(reload_cmd.clone())
                                                {
                                                    Ok(_) => {
                                                        log::debug!(
                                                            "Reload: handle_client_command returned Ok"
                                                        );
                                                        ServerResponse::Ok
                                                    }
                                                    Err(e) => {
                                                        log::error!(
                                                            "Reload: handle_client_command returned error: {e}"
                                                        );
                                                        ServerResponse::Error {
                                                            msg: format!("{e}"),
                                                        }
                                                    }
                                                };
                                                let was_ok = matches!(response, ServerResponse::Ok);
                                                log::debug!(
                                                    "Reload: was_ok={}, elapsed={}ms",
                                                    was_ok,
                                                    reload_start_time.elapsed().as_millis()
                                                );

                                                // Send initial status response
                                                if !send_response(
                                                    &mut stream,
                                                    response,
                                                    &connections,
                                                    &addr,
                                                ) {
                                                    break;
                                                }

                                                // If wait is requested, check readiness and send ReloadResult
                                                if was_ok && wait_flag == Some(true) {
                                                    let timeout_val = timeout.unwrap_or(2000);
                                                    let wait_start = std::time::Instant::now();
                                                    let mut ready = false;
                                                    let mut poll_count = 0u32;

                                                    log::debug!(
                                                        "Reload: Starting wait loop (timeout={}ms)",
                                                        timeout_val
                                                    );

                                                    // Poll for readiness with timeout
                                                    // Note: This blocks the TCP handler thread, but timeout is bounded (default 2s)
                                                    // and readiness should be reached quickly after reload completes
                                                    while wait_start.elapsed().as_millis()
                                                        < timeout_val as u128
                                                    {
                                                        poll_count += 1;
                                                        let elapsed_ms =
                                                            wait_start.elapsed().as_millis();

                                                        if poll_count % 20 == 0 || elapsed_ms > 1000
                                                        {
                                                            log::debug!(
                                                                "Reload: wait loop poll #{}: elapsed={}ms, checking readiness",
                                                                poll_count,
                                                                elapsed_ms
                                                            );
                                                        }

                                                        let k = kanata.lock();
                                                        ready = k.is_ready();
                                                        drop(k);

                                                        if ready {
                                                            log::debug!(
                                                                "Reload: wait loop detected ready=true after {}ms ({} polls)",
                                                                elapsed_ms,
                                                                poll_count
                                                            );
                                                            break;
                                                        }

                                                        if poll_count % 20 == 0 {
                                                            log::debug!(
                                                                "Reload: wait loop - ready=false, continuing to wait"
                                                            );
                                                        }

                                                        // Small sleep to avoid busy-waiting
                                                        std::thread::sleep(
                                                            std::time::Duration::from_millis(50),
                                                        );
                                                    }

                                                    let final_elapsed =
                                                        wait_start.elapsed().as_millis();
                                                    log::debug!(
                                                        "Reload: wait loop completed: ready={}, elapsed={}ms, polls={}",
                                                        ready,
                                                        final_elapsed,
                                                        poll_count
                                                    );

                                                    let result_msg = if ready {
                                                        ServerMessage::ReloadResult {
                                                            request_id,
                                                            ready: true,
                                                            timeout_ms: None,
                                                        }
                                                    } else {
                                                        ServerMessage::ReloadResult {
                                                            request_id,
                                                            ready: false,
                                                            timeout_ms: Some(timeout_val),
                                                        }
                                                    };

                                                    log::debug!(
                                                        "Reload: Sending ReloadResult response"
                                                    );
                                                    // Send ReloadResult details on second line
                                                    if write_with_disconnect_handling(
                                                        &mut stream,
                                                        &result_msg.as_bytes(),
                                                        &addr,
                                                        &connections,
                                                        "Error writing ReloadResult response",
                                                    ) {
                                                        log::debug!(
                                                            "Reload: ReloadResult sent successfully"
                                                        );
                                                        // Flush to ensure immediate delivery
                                                        let _ = stream.flush();
                                                    }

                                                    let total_elapsed =
                                                        reload_start_time.elapsed().as_millis();
                                                    log::info!(
                                                        "Reload: Complete wait path finished: total_elapsed={}ms, wait_elapsed={}ms",
                                                        total_elapsed,
                                                        final_elapsed
                                                    );
                                                } else {
                                                    let total_elapsed =
                                                        reload_start_time.elapsed().as_millis();
                                                    log::debug!(
                                                        "Reload: Non-wait path finished: total_elapsed={}ms",
                                                        total_elapsed
                                                    );
                                                }
                                            }
                                        }
                                        use kanata_parser::keys::*;
                                        wakeup_channel
                                            .send(KeyEvent {
                                                code: OsCode::KEY_RESERVED,
                                                value: KeyValue::WakeUp,
                                            })
                                            .expect("write key event");
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "client sent an invalid message, disconnecting them. Err: {e:?}"
                                        );
                                        // Send proper error response for malformed JSON
                                        let response = ServerResponse::Error {
                                            msg: format!("Failed to deserialize command: {e}"),
                                        };
                                        let _ = stream.write_all(&response.as_bytes());
                                        connections.lock().remove(&addr);
                                        break;
                                    }
                                }
                            }
                            // Clean up connection when client disconnects (loop exits)
                            log::info!("connection closed for {addr}, cleaning up");
                            connections.lock().remove(&addr);
                        });
                    }
                    Err(_) => log::error!("not able to accept client connection"),
                }
            }
        });
    }

    #[cfg(not(feature = "tcp_server"))]
    pub fn start(&mut self, _kanata: Arc<Mutex<Kanata>>) {}
}

#[cfg(feature = "tcp_server")]
pub fn simple_sexpr_to_json_array(exprs: &[SimpleSExpr]) -> serde_json::Value {
    let mut result = Vec::new();

    for expr in exprs.iter() {
        match expr {
            SimpleSExpr::Atom(s) => result.push(serde_json::Value::String(s.clone())),
            SimpleSExpr::List(list) => result.push(simple_sexpr_to_json_array(list)),
        }
    }

    serde_json::Value::Array(result)
}
