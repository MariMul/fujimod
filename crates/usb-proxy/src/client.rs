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
    let text = serde_json::to_string(frame).map_err(err)?;
    if text.len() > MAX_MESSAGE {
        return Err(err("message exceeds limit"));
    }
    match socket {
        Socket::Peer(peer) => peer.send(&text).await,
        Socket::Web(socket) => {
            #[cfg(not(target_arch = "wasm32"))]
            let message = async_tungstenite::tungstenite::Message::Text(text.into());
            #[cfg(target_arch = "wasm32")]
            let message = gloo_net::websocket::Message::Text(text);
            socket.send(message).await.map_err(err)
        }
    }
}
async fn receive(socket: &mut Socket) -> Result<Frame> {
    let text = match socket {
        Socket::Peer(peer) => peer.receive().await?,
        Socket::Web(socket) => loop {
            let message = socket
                .next()
                .await
                .ok_or_else(|| err("disconnected; completion unknown"))?
                .map_err(err)?;
            #[cfg(not(target_arch = "wasm32"))]
            match message {
                async_tungstenite::tungstenite::Message::Text(text) => break text.to_string(),
                async_tungstenite::tungstenite::Message::Ping(_)
                | async_tungstenite::tungstenite::Message::Pong(_) => continue,
                _ => return Err(err("expected text frame")),
            }
            #[cfg(target_arch = "wasm32")]
            match message {
                gloo_net::websocket::Message::Text(text) => break text,
                _ => return Err(err("expected text frame")),
            }
        },
    };
    if text.len() > MAX_MESSAGE {
        return Err(err("message exceeds limit"));
    }
    serde_json::from_str(&text).map_err(err)
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

/// One controller connection. Calls are exclusive and never retried. A dropped
/// or failed call permanently fences this connection; reconnect explicitly.
pub struct Client {
    socket: Socket,
    next_id: u64,
    usable: bool,
    latency: Latency,
    probed_device: String,
    last_probe: Option<Clock>,
}
impl Client {
    pub async fn connect(url: &str, token: &str) -> Result<Self> {
        let ticket = crate::share::Ticket::from_url(url)?;
        let token = ticket.as_ref().map(|t| t.key.as_str()).unwrap_or(token);
        deadline(
            Duration::from_secs(if ticket.is_some() { 30 } else { 10 }),
            async {
                let mut socket = if ticket.is_some() {
                    Socket::Peer(crate::peer::PeerSocket::connect(url).await?)
                } else {
                    #[cfg(not(target_arch = "wasm32"))]
                    let socket = async_tungstenite::smol::connect_async(url)
                        .await
                        .map_err(err)?
                        .0;
                    #[cfg(target_arch = "wasm32")]
                    let socket = WebSocket::open(url).map_err(err)?;
                    Socket::Web(socket)
                };
                send(
                    &mut socket,
                    &Frame::Hello {
                        version: VERSION,
                        role: "controller".into(),
                        token: token.into(),
                    },
                )
                .await?;
                match receive(&mut socket).await? {
                    Frame::Ready {
                        version: VERSION, ..
                    } => Ok(Self {
                        socket,
                        next_id: 0,
                        usable: true,
                        latency: Latency::default(),
                        probed_device: String::new(),
                        last_probe: None,
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
        .await?;
        self.usable = reply.error.is_none();
        Ok(reply)
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
