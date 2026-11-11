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
use std::io::{Read, Write};
#[cfg(feature = "tcp_server")]
use std::net::{TcpListener, TcpStream};
#[cfg(all(feature = "tcp_server", unix))]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(feature = "tcp_server")]
use std::time::Duration;

#[cfg(feature = "tcp_server")]
pub type Connections = Arc<Mutex<HashMap<String, ClientStream>>>;

#[cfg(not(feature = "tcp_server"))]
pub type Connections = ();

/// A connected client, over TCP or a Unix-domain socket.
#[cfg(feature = "tcp_server")]
pub enum ClientStream {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
}

#[cfg(feature = "tcp_server")]
impl ClientStream {
    pub fn try_clone(&self) -> std::io::Result<Self> {
        match self {
            ClientStream::Tcp(stream) => stream.try_clone().map(ClientStream::Tcp),
            #[cfg(unix)]
            ClientStream::Unix(stream) => stream.try_clone().map(ClientStream::Unix),
        }
    }

    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        match self {
            ClientStream::Tcp(stream) => stream.set_write_timeout(timeout),
            #[cfg(unix)]
            ClientStream::Unix(stream) => stream.set_write_timeout(timeout),
        }
    }

    /// A write to a client that has gone away returns `EPIPE` instead of
    /// raising `SIGPIPE`. An embedding process (KeyPath's worker) does not get
    /// Rust's own binary-level `SIGPIPE` ignore, so without this one dead
    /// client kills the whole process mid-write, skipping its cleanup.
    pub fn set_no_sigpipe(&self) -> std::io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            use std::os::fd::AsRawFd;
            let fd = match self {
                ClientStream::Tcp(stream) => stream.as_raw_fd(),
                ClientStream::Unix(stream) => stream.as_raw_fd(),
            };
            let on: libc::c_int = 1;
            let result = unsafe {
                libc::setsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_NOSIGPIPE,
                    (&on as *const libc::c_int).cast(),
                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                )
            };
            if result != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    }

    /// Closes both directions, which also ends the client's reader thread.
    pub fn shutdown(&self) {
        let _ = match self {
            ClientStream::Tcp(stream) => stream.shutdown(std::net::Shutdown::Both),
            #[cfg(unix)]
            ClientStream::Unix(stream) => stream.shutdown(std::net::Shutdown::Both),
        };
    }
}

#[cfg(feature = "tcp_server")]
impl Read for ClientStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            ClientStream::Tcp(stream) => stream.read(buf),
            #[cfg(unix)]
            ClientStream::Unix(stream) => stream.read(buf),
        }
    }
}

#[cfg(feature = "tcp_server")]
impl Write for ClientStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            ClientStream::Tcp(stream) => stream.write(buf),
            #[cfg(unix)]
            ClientStream::Unix(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            ClientStream::Tcp(stream) => stream.flush(),
            #[cfg(unix)]
            ClientStream::Unix(stream) => stream.flush(),
        }
    }
}

/// What a server accepts beyond the protocol itself.
///
/// The default matches upstream kanata. [`ServerPolicy::session`] is for an
/// engine embedded in another app's user session: it refuses the commands that
/// replace the config from a path or move the pointer, caps connections, and
/// drops a client that stops reading instead of letting it stall notifications.
#[cfg(feature = "tcp_server")]
#[derive(Clone, Debug, Default)]
pub struct ServerPolicy {
    /// Refuse `ReloadFile`, `ReloadNext`, `ReloadPrev`, `ReloadNum` and `SetMouse`.
    pub restrict_commands: bool,
    /// Refuse new connections while this many of the same kind (TCP or Unix) are open.
    pub max_connections: Option<usize>,
    /// Applied to every client socket; a notification write that times out drops the client.
    pub write_timeout: Option<Duration>,
}

#[cfg(feature = "tcp_server")]
impl ServerPolicy {
    pub fn session() -> Self {
        Self {
            restrict_commands: true,
            max_connections: Some(16),
            write_timeout: Some(Duration::from_secs(2)),
        }
    }

    /// The command's name when this policy refuses it.
    pub fn refused_command(&self, message: &ClientMessage) -> Option<&'static str> {
        if !self.restrict_commands {
            return None;
        }
        match message {
            ClientMessage::ReloadFile { .. } => Some("ReloadFile"),
            ClientMessage::ReloadNext { .. } => Some("ReloadNext"),
            ClientMessage::ReloadPrev { .. } => Some("ReloadPrev"),
            ClientMessage::ReloadNum { .. } => Some("ReloadNum"),
            ClientMessage::SetMouse { .. } => Some("SetMouse"),
            _ => None,
        }
    }
}

/// Who is on the other end of a Unix-domain connection, read from the socket itself.
#[cfg(all(feature = "tcp_server", unix))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerIdentity {
    pub uid: u32,
    pub pid: i32,
    /// Changes when a PID is reused, so a PID plus its version names one process.
    pub pid_version: u32,
}

#[cfg(all(feature = "tcp_server", unix))]
pub type PeerReader = Arc<dyn Fn(&UnixStream) -> std::io::Result<PeerIdentity> + Send + Sync>;

/// Decides, before a connection receives anything, whether its peer may use the server.
#[cfg(all(feature = "tcp_server", unix))]
#[derive(Clone)]
pub struct PeerCheck {
    expected_uid: u32,
    /// The one process allowed to connect: its PID and PID version.
    expected_owner: Option<(i32, u32)>,
    reader: PeerReader,
}

#[cfg(all(feature = "tcp_server", unix))]
impl PeerCheck {
    pub fn new(expected_uid: u32, expected_owner: Option<(i32, u32)>) -> Self {
        Self {
            expected_uid,
            expected_owner,
            reader: Arc::new(read_peer_identity),
        }
    }

    /// Replaces how the peer is read, so tests can present any identity.
    pub fn with_reader(mut self, reader: PeerReader) -> Self {
        self.reader = reader;
        self
    }

    pub fn verify(&self, stream: &UnixStream) -> Result<PeerIdentity, String> {
        let peer = (self.reader)(stream).map_err(|e| format!("peer identity unavailable: {e}"))?;
        if peer.uid != self.expected_uid {
            return Err(format!(
                "peer uid {} is not {}",
                peer.uid, self.expected_uid
            ));
        }
        if let Some((pid, pid_version)) = self.expected_owner
            && (peer.pid != pid || peer.pid_version != pid_version)
        {
            return Err(format!(
                "peer pid {} (version {}) is not the owner {pid} (version {pid_version})",
                peer.pid, peer.pid_version
            ));
        }
        Ok(peer)
    }
}

/// Reads the connecting process's audit token (`LOCAL_PEERTOKEN`) and checks
/// its uid against `getpeereid`.
#[cfg(all(feature = "tcp_server", target_os = "macos"))]
pub fn read_peer_identity(stream: &UnixStream) -> std::io::Result<PeerIdentity> {
    use std::os::fd::AsRawFd;
    let fd = stream.as_raw_fd();
    let mut token = [0u32; 8];
    let mut len = std::mem::size_of_val(&token) as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_LOCAL,
            libc::LOCAL_PEERTOKEN,
            token.as_mut_ptr().cast(),
            &mut len,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    if len as usize != std::mem::size_of_val(&token) {
        return Err(std::io::Error::other("short audit token"));
    }
    let (mut uid, mut gid) = (0, 0);
    if unsafe { libc::getpeereid(fd, &mut uid, &mut gid) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // audit_token_t: [auid, euid, egid, ruid, rgid, pid, asid, pidversion]
    if token[1] != uid {
        return Err(std::io::Error::other(
            "audit token and getpeereid disagree on uid",
        ));
    }
    Ok(PeerIdentity {
        uid,
        pid: token[5] as i32,
        pid_version: token[7],
    })
}

#[cfg(all(feature = "tcp_server", unix, not(target_os = "macos")))]
pub fn read_peer_identity(_stream: &UnixStream) -> std::io::Result<PeerIdentity> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "peer identity is only implemented on macOS",
    ))
}

/// The listener's filesystem path. A socket bound with the full
/// `sockaddr_un` length (as Swift's `MemoryLayout<sockaddr_un>.size` does)
/// reads back with trailing NUL bytes, which no file call accepts, so the
/// path ends at the first NUL.
#[cfg(all(feature = "tcp_server", unix))]
fn bound_path(addr: &std::os::unix::net::SocketAddr) -> Option<std::path::PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let bytes = addr.as_pathname()?.as_os_str().as_bytes();
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    (end > 0).then(|| std::ffi::OsStr::from_bytes(&bytes[..end]).into())
}

/// Ids of Unix-socket clients in [`Connections`] start with this.
#[cfg(feature = "tcp_server")]
const UNIX_CLIENT_PREFIX: &str = "unix-";

/// Open clients of one kind. TCP and Unix clients are counted apart, so
/// processes that can reach loopback TCP cannot use up the owner's socket.
#[cfg(feature = "tcp_server")]
fn open_clients<'a>(ids: impl Iterator<Item = &'a String>, unix: bool) -> usize {
    ids.filter(|id| id.starts_with(UNIX_CLIENT_PREFIX) == unix)
        .count()
}

/// Stops a Unix-socket server from accepting, removes its socket file and
/// closes its open connections.
#[cfg(all(feature = "tcp_server", unix))]
#[derive(Clone)]
pub struct ControlHandle {
    closed: Arc<std::sync::atomic::AtomicBool>,
    path: Option<std::path::PathBuf>,
    connections: Connections,
}

#[cfg(all(feature = "tcp_server", unix))]
impl ControlHandle {
    pub fn close(&self) {
        use std::sync::atomic::Ordering;
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(path) = &self.path {
            // Wake the blocked accept so it sees `closed`, then remove the path.
            let _ = UnixStream::connect(path);
            let _ = std::fs::remove_file(path);
        }
        // Only this socket's clients; a TCP listener sharing the set keeps its own.
        self.connections.lock().retain(|id, client| {
            let control = id.starts_with(UNIX_CLIENT_PREFIX);
            if control {
                client.shutdown();
            }
            !control
        });
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::Acquire)
    }
}

#[cfg(feature = "tcp_server")]
use kanata_parser::custom_action::FakeKeyAction;

#[cfg(feature = "tcp_server")]
fn send_response(
    stream: &mut ClientStream,
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

#[cfg(feature = "tcp_server")]
fn to_action(val: FakeKeyActionMessage) -> FakeKeyAction {
    match val {
        FakeKeyActionMessage::Press => FakeKeyAction::Press,
        FakeKeyActionMessage::Release => FakeKeyAction::Release,
        FakeKeyActionMessage::Tap => FakeKeyAction::Tap,
        FakeKeyActionMessage::Toggle => FakeKeyAction::Toggle,
    }
}

/// Handles reload commands with optional wait/timeout for completion confirmation.
/// Returns false if the connection should be closed, true otherwise.
#[cfg(feature = "tcp_server")]
fn handle_reload_with_wait(
    reload_cmd: ClientMessage,
    wait: Option<bool>,
    timeout_ms: Option<u64>,
    stream: &mut ClientStream,
    kanata: &Arc<Mutex<Kanata>>,
    connections: &Connections,
    addr: &str,
) -> bool {
    let (response, reload_ok) = match kanata.lock().handle_client_command(reload_cmd) {
        Ok(_) => (ServerResponse::Ok, true),
        Err(e) => (
            ServerResponse::Error {
                msg: format!("{e}"),
            },
            false,
        ),
    };
    if !send_response(stream, response, connections, addr) {
        return false;
    }

    // If wait flag is set and reload succeeded, poll for completion
    if reload_ok && wait.unwrap_or(false) {
        let timeout_ms = timeout_ms.unwrap_or(5000);
        let start = std::time::Instant::now();
        let timeout_duration = std::time::Duration::from_millis(timeout_ms);

        while start.elapsed() < timeout_duration {
            if kanata.lock().is_reload_complete() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let timed_out = start.elapsed() >= timeout_duration;

        let ok = kanata.lock().last_reload_succeeded();
        let msg = ServerMessage::ReloadResult {
            ok,
            timeout_ms: if timed_out { Some(timeout_ms) } else { None },
        };
        if let Err(err) = stream.write_all(&msg.as_bytes()) {
            log::error!("Error writing ReloadResult: {err}");
            connections.lock().remove(addr);
            return false;
        }
        let _ = stream.flush();
    }
    true
}

#[cfg(feature = "tcp_server")]
pub struct TcpServer {
    pub address: SocketAddr,
    pub connections: Connections,
    pub wakeup_channel: Sender<KeyEvent>,
    pub policy: ServerPolicy,
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
            policy: ServerPolicy::default(),
        }
    }

    #[cfg(not(feature = "tcp_server"))]
    pub fn new(_address: SocketAddr, _wakeup_channel: Sender<KeyEvent>) -> Self {
        Self { connections: () }
    }

    #[cfg(feature = "tcp_server")]
    pub fn with_policy(mut self, policy: ServerPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Binds `address` and serves it. Panics when the address is unavailable;
    /// an embedder that must not panic binds itself and calls [`TcpServer::start_tcp`].
    #[cfg(feature = "tcp_server")]
    pub fn start(&mut self, kanata: Arc<Mutex<Kanata>>) {
        let listener = TcpListener::bind(self.address).expect("TCP server starts");
        self.start_tcp(listener, kanata);
    }

    /// Serves an already-bound TCP listener.
    #[cfg(feature = "tcp_server")]
    pub fn start_tcp(&mut self, listener: TcpListener, kanata: Arc<Mutex<Kanata>>) {
        let connections = self.connections.clone();
        let wakeup_channel = self.wakeup_channel.clone();
        let policy = self.policy.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let addr = match stream.peer_addr() {
                            Ok(addr) => addr.to_string(),
                            Err(e) => {
                                log::warn!("failed to get peer address, using fallback: {e:?}");
                                format!("unknown_{}", std::ptr::addr_of!(stream) as usize)
                            }
                        };
                        accept_client(
                            ClientStream::Tcp(stream),
                            addr,
                            &kanata,
                            &connections,
                            &wakeup_channel,
                            &policy,
                        );
                    }
                    Err(_) => log::error!("not able to accept client connection"),
                }
            }
        });
    }

    /// Serves an already-bound Unix-domain listener. Every connection is
    /// checked with `peer_check` before it is answered or joins the
    /// notification set; a refused connection is closed without a byte.
    #[cfg(all(feature = "tcp_server", unix))]
    pub fn start_unix(
        &mut self,
        listener: UnixListener,
        peer_check: PeerCheck,
        kanata: Arc<Mutex<Kanata>>,
    ) -> ControlHandle {
        use std::sync::atomic::{AtomicU64, Ordering};
        let handle = ControlHandle {
            closed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            path: listener
                .local_addr()
                .ok()
                .and_then(|addr| bound_path(&addr)),
            connections: self.connections.clone(),
        };
        let accept_handle = handle.clone();
        let connections = self.connections.clone();
        let wakeup_channel = self.wakeup_channel.clone();
        let policy = self.policy.clone();
        std::thread::spawn(move || {
            let next_id = AtomicU64::new(0);
            for stream in listener.incoming() {
                if accept_handle.is_closed() {
                    log::info!("control socket closed; no longer accepting");
                    break;
                }
                match stream {
                    Ok(stream) => match peer_check.verify(&stream) {
                        Ok(peer) => {
                            let id = next_id.fetch_add(1, Ordering::Relaxed);
                            accept_client(
                                ClientStream::Unix(stream),
                                format!("{UNIX_CLIENT_PREFIX}{id}-pid-{}", peer.pid),
                                &kanata,
                                &connections,
                                &wakeup_channel,
                                &policy,
                            );
                        }
                        Err(reason) => {
                            log::warn!("refused control connection: {reason}");
                            let _ = stream.shutdown(std::net::Shutdown::Both);
                        }
                    },
                    Err(e) => log::error!("not able to accept control connection: {e}"),
                }
            }
        });
        handle
    }

    #[cfg(not(feature = "tcp_server"))]
    pub fn start(&mut self, _kanata: Arc<Mutex<Kanata>>) {}
}

/// Admits one connection: applies the policy, tells the client the current
/// layer, adds it to the notification set and serves its commands.
#[cfg(feature = "tcp_server")]
fn accept_client(
    mut stream: ClientStream,
    addr: String,
    kanata: &Arc<Mutex<Kanata>>,
    connections: &Connections,
    wakeup_channel: &Sender<KeyEvent>,
    policy: &ServerPolicy,
) {
    let unix = addr.starts_with(UNIX_CLIENT_PREFIX);
    if let Some(max) = policy.max_connections
        && open_clients(connections.lock().keys(), unix) >= max
    {
        log::warn!("refused client {addr}: {max} connections already open");
        stream.shutdown();
        return;
    }
    if let Err(e) = stream.set_no_sigpipe() {
        log::warn!("could not disable SIGPIPE for {addr}, dropping it: {e:?}");
        stream.shutdown();
        return;
    }
    if let Err(e) = stream.set_write_timeout(policy.write_timeout) {
        log::warn!("could not set a write timeout for {addr}, dropping it: {e:?}");
        stream.shutdown();
        return;
    }
    {
        let k = kanata.lock();
        log::info!(
            "new client connection, sending initial LayerChange event to inform them of current layer"
        );
        if let Err(e) = stream.write_all(
            &ServerMessage::LayerChange {
                new: k.layer_info[k.layout.b().current_layer()].name.clone(),
            }
            .as_bytes(),
        ) {
            log::warn!("failed to write to stream, dropping it: {e:?}");
            return;
        }
    }
    let (notify, read) = match (stream.try_clone(), stream.try_clone()) {
        (Ok(notify), Ok(read)) => (notify, read),
        _ => {
            log::warn!("could not clone client stream {addr}, dropping it");
            stream.shutdown();
            return;
        }
    };
    connections.lock().insert(addr.clone(), notify);
    let reader = serde_json::Deserializer::from_reader(read).into_iter::<ClientMessage>();
    log::info!("listening for incoming messages {addr}");

    let connections = connections.clone();
    let kanata = kanata.clone();
    let wakeup_channel = wakeup_channel.clone();
    let policy = policy.clone();
    std::thread::spawn(move || {
        serve_commands(
            stream,
            reader,
            addr.clone(),
            &kanata,
            &connections,
            &wakeup_channel,
            &policy,
        );
        // However the client ended, it no longer receives notifications.
        if let Some(client) = connections.lock().remove(&addr) {
            client.shutdown();
        }
    });
}

#[cfg(feature = "tcp_server")]
fn serve_commands(
    mut stream: ClientStream,
    reader: impl Iterator<Item = Result<ClientMessage, serde_json::Error>>,
    addr: String,
    kanata: &Arc<Mutex<Kanata>>,
    connections: &Connections,
    wakeup_channel: &Sender<KeyEvent>,
    policy: &ServerPolicy,
) {
    use kanata_parser::cfg::FAKE_KEY_ROW;

    use crate::kanata::handle_fakekey_action;

    for v in reader {
        match v {
            Ok(event) => {
                log::debug!("tcp server received command: {:?}", event);
                if let Some(name) = policy.refused_command(&event) {
                    log::warn!("refused {name}: not available in this build");
                    let response = ServerResponse::Error {
                        msg: format!("{name} is not available in this build"),
                    };
                    if !send_response(&mut stream, response, connections, &addr) {
                        break;
                    }
                    continue;
                }
                match event {
                    ClientMessage::ChangeLayer { new } => {
                        kanata.lock().change_layer(new);
                    }
                    ClientMessage::RequestLayerNames {} => {
                        let msg = ServerMessage::LayerNames {
                            names: kanata
                                .lock()
                                .layer_info
                                .iter()
                                .map(|info| info.name.clone())
                                .collect::<Vec<_>>(),
                        };
                        match stream.write_all(&msg.as_bytes()) {
                            Ok(_) => {}
                            Err(err) => log::error!("server could not send response: {err}"),
                        }
                    }
                    ClientMessage::RequestFakeKeyNames {} => {
                        let msg = ServerMessage::FakeKeyNames {
                            names: kanata
                                .lock()
                                .virtual_keys
                                .keys()
                                .cloned()
                                .collect::<Vec<_>>(),
                        };
                        match stream.write_all(&msg.as_bytes()) {
                            Ok(_) => {}
                            Err(err) => log::error!("server could not send response: {err}"),
                        }
                    }
                    ClientMessage::ActOnFakeKey { name, action } => {
                        let mut k = kanata.lock();
                        let index = match k.virtual_keys.get(&name) {
                            Some(index) => Some(*index as u16),
                            None => {
                                if let Err(e) = stream.write_all(
                                    &ServerMessage::Error {
                                        msg: format!("unknown virtual/fake key: {name}"),
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
                            log::info!("tcp server fake-key action: {name},{action:?}");
                            handle_fakekey_action(
                                to_action(action),
                                k.layout.bm(),
                                FAKE_KEY_ROW,
                                index,
                            );
                        }
                        drop(k);
                    }
                    ClientMessage::SetMouse { x, y } => {
                        log::info!("tcp server SetMouse action: x {x} y {y}");
                        match kanata.lock().kbd_out.set_mouse(x, y) {
                            Ok(_) => {
                                log::info!("sucessfully did set mouse position to: x {x} y {y}");
                            }
                            Err(e) => {
                                log::error!("Failed to set mouse position: {}", e);
                            }
                        }
                    }
                    ClientMessage::RequestCurrentLayerInfo {} => {
                        let mut k = kanata.lock();
                        let cur_layer = k.layout.bm().current_layer();
                        let msg = ServerMessage::CurrentLayerInfo {
                            name: k.layer_info[cur_layer].name.clone(),
                            cfg_text: k.layer_info[cur_layer].cfg_text.clone(),
                        };
                        drop(k);
                        match stream.write_all(&msg.as_bytes()) {
                            Ok(_) => {}
                            Err(err) => log::error!(
                                "Error writing response to RequestCurrentLayerInfo: {err}"
                            ),
                        }
                    }
                    ClientMessage::RequestCurrentLayerName {} => {
                        let mut k = kanata.lock();
                        let cur_layer = k.layout.bm().current_layer();
                        let msg = ServerMessage::CurrentLayerName {
                            name: k.layer_info[cur_layer].name.clone(),
                        };
                        drop(k);
                        match stream.write_all(&msg.as_bytes()) {
                            Ok(_) => {}
                            Err(err) => log::error!(
                                "Error writing response to RequestCurrentLayerName: {err}"
                            ),
                        }
                    }
                    ClientMessage::RequestInputGrab {} => {
                        // Answer with the last authoritative grab status
                        // cached by `emit_input_grab`. If the grab attempt
                        // has not resolved yet (nothing cached), send no
                        // response: we never fabricate a status, and the
                        // client falls back to its own detection on silence.
                        let k = kanata.lock();
                        let cached = k.last_input_grab.clone();
                        drop(k);
                        if let Some((active, devices, reason)) = cached {
                            let msg = ServerMessage::InputGrab {
                                active,
                                devices,
                                reason,
                            };
                            match stream.write_all(&msg.as_bytes()) {
                                Ok(_) => {}
                                Err(err) => {
                                    log::error!("Error writing response to RequestInputGrab: {err}")
                                }
                            }
                        }
                    }
                    // New command: Hello - capability detection
                    ClientMessage::Hello {} => {
                        let version = env!("CARGO_PKG_VERSION").to_string();
                        let mut capabilities = vec![
                            "reload".to_string(),
                            "layer-names".to_string(),
                            "fake-key-names".to_string(),
                            "layer-change".to_string(),
                            "hold-activated".to_string(),
                            "tap-activated".to_string(),
                            "chord-resolved".to_string(),
                            "tap-dance-resolved".to_string(),
                            "current-layer-name".to_string(),
                            "current-layer-info".to_string(),
                            "fake-key".to_string(),
                            "set-mouse".to_string(),
                            "key-input".to_string(),
                            "input-grab".to_string(),
                        ];
                        if policy.restrict_commands {
                            capabilities.retain(|capability| capability != "set-mouse");
                        }
                        let msg = ServerMessage::HelloOk {
                            version,
                            protocol: 1,
                            capabilities,
                        };
                        match stream.write_all(&msg.as_bytes()) {
                            Ok(_) => {
                                let _ = stream.flush();
                            }
                            Err(err) => {
                                log::error!("Error writing HelloOk response: {err}");
                                connections.lock().remove(&addr);
                                break;
                            }
                        }
                    }
                    // Reload commands with optional wait/timeout
                    ClientMessage::Reload { wait, timeout_ms } => {
                        log::info!("tcp server Reload action");
                        if !handle_reload_with_wait(
                            ClientMessage::Reload { wait, timeout_ms },
                            wait,
                            timeout_ms,
                            &mut stream,
                            kanata,
                            connections,
                            &addr,
                        ) {
                            break;
                        }
                    }
                    ClientMessage::ReloadNext { wait, timeout_ms } => {
                        log::info!("tcp server ReloadNext action");
                        if !handle_reload_with_wait(
                            ClientMessage::ReloadNext { wait, timeout_ms },
                            wait,
                            timeout_ms,
                            &mut stream,
                            kanata,
                            connections,
                            &addr,
                        ) {
                            break;
                        }
                    }
                    ClientMessage::ReloadPrev { wait, timeout_ms } => {
                        log::info!("tcp server ReloadPrev action");
                        if !handle_reload_with_wait(
                            ClientMessage::ReloadPrev { wait, timeout_ms },
                            wait,
                            timeout_ms,
                            &mut stream,
                            kanata,
                            connections,
                            &addr,
                        ) {
                            break;
                        }
                    }
                    ClientMessage::ReloadNum {
                        index,
                        wait,
                        timeout_ms,
                    } => {
                        log::info!("tcp server ReloadNum action: index {index}");
                        if !handle_reload_with_wait(
                            ClientMessage::ReloadNum {
                                index,
                                wait,
                                timeout_ms,
                            },
                            wait,
                            timeout_ms,
                            &mut stream,
                            kanata,
                            connections,
                            &addr,
                        ) {
                            break;
                        }
                    }
                    ClientMessage::ReloadFile {
                        path,
                        wait,
                        timeout_ms,
                    } => {
                        log::info!("tcp server ReloadFile action: path {path}");
                        if !handle_reload_with_wait(
                            ClientMessage::ReloadFile {
                                path,
                                wait,
                                timeout_ms,
                            },
                            wait,
                            timeout_ms,
                            &mut stream,
                            kanata,
                            connections,
                            &addr,
                        ) {
                            break;
                        }
                    }
                }
                use kanata_parser::keys::*;
                wakeup_channel
                    .send(KeyEvent::new(OsCode::KEY_RESERVED, KeyValue::WakeUp))
                    .expect("write key event");
            }
            Err(e) => {
                log::warn!("client sent an invalid message, disconnecting them. Err: {e:?}");
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

#[cfg(all(test, feature = "tcp_server", unix))]
mod control_tests {
    use super::*;

    fn identity(uid: u32, pid: i32, pid_version: u32) -> PeerIdentity {
        PeerIdentity {
            uid,
            pid,
            pid_version,
        }
    }

    fn check_with(
        expected_owner: Option<(i32, u32)>,
        presented: PeerIdentity,
    ) -> Result<PeerIdentity, String> {
        let (stream, _other) = UnixStream::pair().expect("socket pair");
        PeerCheck::new(501, expected_owner)
            .with_reader(Arc::new(move |_| Ok(presented)))
            .verify(&stream)
    }

    #[test]
    fn the_owner_is_admitted() {
        assert_eq!(
            check_with(Some((42, 7)), identity(501, 42, 7)),
            Ok(identity(501, 42, 7))
        );
    }

    #[test]
    fn another_user_is_refused() {
        assert!(check_with(Some((42, 7)), identity(502, 42, 7)).is_err());
        assert!(check_with(None, identity(502, 42, 7)).is_err());
    }

    #[test]
    fn another_process_of_the_same_user_is_refused() {
        assert!(check_with(Some((42, 7)), identity(501, 43, 7)).is_err());
    }

    /// The owner exited and its PID number was reused before the exit watch
    /// fired: same PID, different PID version. It must not pass.
    #[test]
    fn a_reused_owner_pid_is_refused() {
        assert!(check_with(Some((42, 7)), identity(501, 42, 8)).is_err());
    }

    #[test]
    fn an_unreadable_peer_is_refused() {
        let (stream, _other) = UnixStream::pair().expect("socket pair");
        let result = PeerCheck::new(501, None)
            .with_reader(Arc::new(|_| Err(std::io::Error::other("no token"))))
            .verify(&stream);
        assert!(result.is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_peer_token_names_this_process() {
        let (a, _b) = UnixStream::pair().expect("socket pair");
        let peer = read_peer_identity(&a).expect("audit token");
        assert_eq!(peer.pid, std::process::id() as i32);
        assert_eq!(peer.uid, unsafe { libc::getuid() });
        assert_ne!(peer.pid_version, 0, "a live process has a PID version");
        // And the real reader admits this process as its own owner.
        PeerCheck::new(peer.uid, Some((peer.pid, peer.pid_version)))
            .verify(&a)
            .expect("owner admitted");
    }

    /// Writing to a peer that has gone away must fail, not raise SIGPIPE
    /// (which would kill this test process, as it killed the worker).
    #[cfg(target_os = "macos")]
    #[test]
    fn a_write_to_a_closed_peer_returns_an_error_instead_of_sigpipe() {
        let (a, b) = UnixStream::pair().expect("socket pair");
        let mut client = ClientStream::Unix(a);
        client.set_no_sigpipe().expect("SO_NOSIGPIPE");
        drop(b);
        let mut failed = false;
        for _ in 0..4 {
            if client.write_all(&[0u8; 65536]).is_err() {
                failed = true;
                break;
            }
        }
        assert!(failed, "writes to a closed peer must report an error");
    }

    /// Bound with the whole sockaddr_un length, as KeyPath's Swift code does:
    /// the path read back must still name the file.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_full_length_bind_still_yields_the_socket_path() {
        use std::os::fd::FromRawFd;
        let dir = std::env::temp_dir().join(format!("kp-fullbind-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("e.sock");
        let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
        assert!(fd >= 0);
        let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        address.sun_family = libc::AF_UNIX as libc::sa_family_t;
        for (slot, byte) in address
            .sun_path
            .iter_mut()
            .zip(path.to_str().unwrap().bytes())
        {
            *slot = byte as libc::c_char;
        }
        let full = std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
        assert_eq!(
            unsafe { libc::bind(fd, (&address as *const libc::sockaddr_un).cast(), full) },
            0
        );
        assert_eq!(unsafe { libc::listen(fd, 1) }, 0);
        let listener = unsafe { UnixListener::from_raw_fd(fd) };
        assert_eq!(
            bound_path(&listener.local_addr().unwrap()),
            Some(path.clone())
        );
        drop(listener);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn tcp_clients_do_not_count_against_the_unix_cap() {
        let ids: Vec<String> = (0..16)
            .map(|i| format!("127.0.0.1:{}", 5000 + i))
            .chain(["unix-0-pid-42".to_string()])
            .collect();
        assert_eq!(open_clients(ids.iter(), false), 16);
        assert_eq!(open_clients(ids.iter(), true), 1);
        let cap = ServerPolicy::session().max_connections.unwrap();
        assert!(
            open_clients(ids.iter(), true) < cap,
            "TCP clients must not lock out the owner"
        );
    }

    #[test]
    fn the_session_policy_refuses_path_reloads_and_pointer_moves() {
        let session = ServerPolicy::session();
        let refused = [
            ClientMessage::ReloadFile {
                path: "/tmp/x.kbd".into(),
                wait: None,
                timeout_ms: None,
            },
            ClientMessage::ReloadNext {
                wait: None,
                timeout_ms: None,
            },
            ClientMessage::ReloadPrev {
                wait: None,
                timeout_ms: None,
            },
            ClientMessage::ReloadNum {
                index: 1,
                wait: None,
                timeout_ms: None,
            },
            ClientMessage::SetMouse { x: 1, y: 1 },
        ];
        for message in &refused {
            assert!(session.refused_command(message).is_some(), "{message:?}");
            assert!(
                ServerPolicy::default().refused_command(message).is_none(),
                "{message:?}"
            );
        }
        let allowed = [
            ClientMessage::Reload {
                wait: Some(true),
                timeout_ms: None,
            },
            ClientMessage::Hello {},
            ClientMessage::ChangeLayer { new: "base".into() },
            ClientMessage::ActOnFakeKey {
                name: "x".into(),
                action: FakeKeyActionMessage::Tap,
            },
        ];
        for message in &allowed {
            assert!(session.refused_command(message).is_none(), "{message:?}");
        }
    }
}
