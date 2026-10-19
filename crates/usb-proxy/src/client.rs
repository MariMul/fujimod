use crate::{
    Error, Result,
    latency::{Clock, Latency},
    protocol::*,
};
#[cfg(target_arch = "wasm32")]
use futures_util::SinkExt;
use futures_util::StreamExt;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
type WebSocket = async_tungstenite::WebSocketStream<async_tungstenite::smol::ConnectStream>;
#[cfg(target_arch = "wasm32")]
type WebSocket = gloo_net::websocket::futures::WebSocket;

fn err(e: impl std::fmt::Display) -> Error {
    Error(e.to_string())
}
enum Socket {
    Web(WebSocket),
    Peer(crate::peer::PeerSocket),
}
async fn send(socket: &mut Socket, frame: &Frame) -> Result<()> {
    let bytes = frame.encode()?;
    if bytes.len() > MAX_MESSAGE {
        return Err(err("message exceeds limit"));
    }
    match socket {
        Socket::Peer(peer) => peer.send(bytes).await,
        Socket::Web(socket) => {
            #[cfg(not(target_arch = "wasm32"))]
            let message = async_tungstenite::tungstenite::Message::Binary(bytes.into());
            #[cfg(target_arch = "wasm32")]
            let message = gloo_net::websocket::Message::Bytes(bytes);
            socket.send(message).await.map_err(err)
        }
    }
}
async fn receive(socket: &mut Socket) -> Result<Frame> {
    let bytes = match socket {
        Socket::Peer(peer) => peer.receive().await?,
        Socket::Web(socket) => loop {
            let message = socket
                .next()
                .await
                .ok_or_else(|| err("disconnected; completion unknown"))?
                .map_err(err)?;
            #[cfg(not(target_arch = "wasm32"))]
            match message {
                async_tungstenite::tungstenite::Message::Binary(bytes) => break bytes.to_vec(),
                async_tungstenite::tungstenite::Message::Ping(_)
                | async_tungstenite::tungstenite::Message::Pong(_) => continue,
                _ => return Err(err("expected binary frame")),
            }
            #[cfg(target_arch = "wasm32")]
            match message {
                gloo_net::websocket::Message::Bytes(bytes) => break bytes,
                _ => return Err(err("expected binary frame")),
            }
        },
    };
    if bytes.len() > MAX_MESSAGE {
        return Err(err("message exceeds limit"));
    }
    Frame::decode(&bytes)
}
async fn deadline<T>(
    duration: Duration,
    task: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    futures_lite::future::or(task, async {
        futures_timer::Delay::new(duration).await;
        Err(err("deadline expired; completion unknown"))
    })
    .await
}

/// The connect reason shown to the device owner: FUJIMOD_USB_PROXY_REASON, or
/// this process's command line.
fn default_reason() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    return None;
    #[cfg(not(target_arch = "wasm32"))]
    {
        let text = std::env::var("FUJIMOD_USB_PROXY_REASON")
            .unwrap_or_else(|_| std::env::args().collect::<Vec<_>>().join(" "));
        (!text.is_empty()).then(|| bounded_reason(&text))
    }
}

/// Receives each connection step as it completes, before any USB request.
pub type Progress = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

/// One controller connection. Calls are exclusive and never retried. A dropped
/// or failed call permanently fences this connection; reconnect explicitly.
pub struct Client {
    socket: Socket,
    next_id: u64,
    usable: bool,
    latency: Latency,
    probed_device: String,
    last_probe: Option<Clock>,
    /// The error that fenced this connection, sent as the disconnect reason.
    failure: Option<String>,
}
impl Client {
    pub async fn connect(url: &str, token: &str) -> Result<Self> {
        Self::connect_with_progress(url, token, std::sync::Arc::new(|_: &str| {})).await
    }
    /// Connect, reporting each step. A failure names the last step reached, so
    /// signaling, WebRTC and the proxy handshake can be told apart.
    pub async fn connect_with_progress(url: &str, token: &str, progress: Progress) -> Result<Self> {
        let reached = std::sync::Arc::new(std::sync::Mutex::new(String::from("started")));
        let report: Progress = {
            let reached = reached.clone();
            std::sync::Arc::new(move |step: &str| {
                *reached.lock().unwrap() = step.to_owned();
                progress(step);
            })
        };
        Self::connect_reporting(url, token, report)
            .await
            .map_err(|e| {
                err(format!(
                    "{} (last step reached: {})",
                    e.0,
                    reached.lock().unwrap()
                ))
            })
    }
    async fn connect_reporting(url: &str, token: &str, progress: Progress) -> Result<Self> {
        let ticket = crate::share::Ticket::from_url(url)?;
        let token = ticket.as_ref().map(|t| t.key.as_str()).unwrap_or(token);
        deadline(
            Duration::from_secs(if ticket.is_some() { 30 } else { 10 }),
            async {
                let mut socket = if ticket.is_some() {
                    Socket::Peer(crate::peer::PeerSocket::connect(url, &progress).await?)
                } else {
                    #[cfg(not(target_arch = "wasm32"))]
                    let socket = async_tungstenite::smol::connect_async(url)
                        .await
                        .map_err(err)?
                        .0;
                    #[cfg(target_arch = "wasm32")]
                    let socket = WebSocket::open(url).map_err(err)?;
                    progress("relay WebSocket connected");
                    Socket::Web(socket)
                };
                send(
                    &mut socket,
                    &Frame::Hello {
                        version: VERSION,
                        role: "controller".into(),
                        token: token.into(),
                        reason: default_reason(),
                    },
                )
                .await?;
                progress("proxy handshake sent");
                match receive(&mut socket).await? {
                    Frame::Ready {
                        version: VERSION, ..
                    } => Ok({
                        progress("proxy handshake ready");
                        Self {
                            socket,
                            next_id: 0,
                            usable: true,
                            latency: Latency::default(),
                            probed_device: String::new(),
                            last_probe: None,
                            failure: None,
                        }
                    }),
                    _ => Err(err("relay rejected protocol handshake")),
                }
            },
        )
        .await
    }
    pub async fn request(
        &mut self,
        device: &str,
        operation: Operation,
        timeout: Duration,
    ) -> Result<Completion> {
        operation.validate()?;
        if !self.usable {
            return Err(err("connection fenced; completion unknown"));
        }
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(err("timeout must be 1–120000 ms"));
        }
        // Probe the full route before USB, then refresh while the connection is
        // active. A new route gets its own samples; no camera command is sent.
        if !matches!(operation, Operation::List | Operation::Probe)
            && (self.probed_device != device
                || self
                    .last_probe
                    .as_ref()
                    .is_none_or(|clock| clock.elapsed() >= Duration::from_secs(5)))
        {
            let initial = self.probed_device != device;
            if initial {
                self.latency = Latency::default();
            }
            for _ in 0..if initial { 3 } else { 1 } {
                let started = Clock::new();
                let completion = self
                    .exchange(device, Operation::Probe, Duration::from_secs(5))
                    .await?;
                if let Some(error) = completion.error {
                    return Err(err(error));
                }
                self.latency.record(started.elapsed());
            }
            self.probed_device = device.into();
            self.last_probe = Some(Clock::new());
        }
        self.exchange(device, operation, timeout).await
    }
    async fn exchange(
        &mut self,
        device: &str,
        operation: Operation,
        timeout: Duration,
    ) -> Result<Completion> {
        // Set before the first await: cancellation never enables another transfer.
        self.usable = false;
        self.next_id += 1;
        let id = self.next_id;
        // The relay has its own exporter wait. Reserve both that allowance
        // and the full controller-to-exporter RTT outside the USB budget.
        let reply = deadline(
            timeout + self.latency.allowance() * 2 + Duration::from_secs(2),
            async {
                send(
                    &mut self.socket,
                    &Frame::Request {
                        id,
                        device: device.into(),
                        operation,
                        timeout_ms: timeout.as_millis().max(1) as u64,
                    },
                )
                .await?;
                match receive(&mut self.socket).await? {
                    Frame::Reply {
                        id: received,
                        completion,
                    } if received == id => Ok(completion),
                    _ => Err(err("mismatched reply")),
                }
            },
        )
        .await
        .inspect_err(|e| self.failure = Some(e.0.clone()))?;
        self.usable = reply.error.is_none();
        if let Some(error) = &reply.error {
            self.failure = Some(error.clone());
        }
        Ok(reply)
    }
    /// Send the owner a Status or Attention frame; the owner does not reply.
    pub async fn notify_owner(&mut self, frame: Frame) -> Result<()> {
        if !matches!(
            frame,
            Frame::Status { .. }
                | Frame::Attention { .. }
                | Frame::Activity { .. }
                | Frame::ProgressStart { .. }
                | Frame::ProgressUpdate { .. }
                | Frame::ProgressDone
        ) {
            return Err(err("only status, attention, activity and progress frames are notifications"));
        }
        if !self.usable {
            return Err(err("connection fenced; completion unknown"));
        }
        send(&mut self.socket, &frame).await
    }
    pub async fn devices(&mut self) -> Result<Vec<Device>> {
        let completion = self
            .request("", Operation::List, Duration::from_secs(5))
            .await?;
        if let Some(error) = completion.error {
            return Err(err(error));
        }
        Ok(completion.devices)
    }
    /// Discovery owns only descriptors. Opening uses a fresh controller socket.
    pub async fn discover(url: &str, token: &str) -> Result<Vec<RemoteDevice>> {
        let mut client = Self::connect(url, token).await?;
        Ok(client
            .devices()
            .await?
            .into_iter()
            .map(|descriptor| RemoteDevice {
                descriptor,
                url: url.into(),
                token: token.into(),
            })
            .collect())
    }
}

/// Tell the owner why this controller is leaving: "done", or the error that
/// fenced the connection. Best effort; a lost goodbye shows as a lost connection.
#[cfg(not(target_arch = "wasm32"))]
impl Drop for Client {
    fn drop(&mut self) {
        if let Socket::Peer(peer) = &mut self.socket {
            let reason = match &self.failure {
                Some(error) => bounded_reason(&format!("ended after error: {error}")),
                None => "done".to_owned(),
            };
            if let Ok(bytes) = (Frame::Goodbye { reason }).encode() {
                peer.goodbye(bytes);
            }
        }
    }
}

#[derive(Clone)]
pub struct RemoteDevice {
    pub descriptor: Device,
    url: String,
    token: String,
}
impl std::fmt::Debug for RemoteDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteDevice")
            .field("descriptor", &self.descriptor)
            .finish_non_exhaustive()
    }
}
impl RemoteDevice {
    pub async fn connect(&self) -> Result<Client> {
        Client::connect(&self.url, &self.token).await
    }
}
