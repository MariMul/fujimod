use crate::{Error, Result, protocol::*};
#[cfg(target_arch = "wasm32")]
use futures_util::SinkExt;
use futures_util::StreamExt;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
type Socket = async_tungstenite::WebSocketStream<async_tungstenite::smol::ConnectStream>;
#[cfg(target_arch = "wasm32")]
type Socket = gloo_net::websocket::futures::WebSocket;

fn err(e: impl std::fmt::Display) -> Error {
    Error(e.to_string())
}
async fn send(socket: &mut Socket, frame: &Frame) -> Result<()> {
    let json = serde_json::to_string(frame).map_err(err)?;
    let json_len = json.len();
    #[cfg(not(target_arch = "wasm32"))]
    let message = async_tungstenite::tungstenite::Message::Text(json.into());
    #[cfg(target_arch = "wasm32")]
    let message = gloo_net::websocket::Message::Text(json);
    if json_len > MAX_MESSAGE {
        return Err(err("message exceeds limit"));
    }
    socket.send(message).await.map_err(err)
}
async fn receive(socket: &mut Socket) -> Result<Frame> {
    loop {
        let message = socket
            .next()
            .await
            .ok_or_else(|| err("disconnected; completion unknown"))?
            .map_err(err)?;
        #[cfg(not(target_arch = "wasm32"))]
        let text = match message {
            async_tungstenite::tungstenite::Message::Text(text) => text.to_string(),
            async_tungstenite::tungstenite::Message::Ping(_)
            | async_tungstenite::tungstenite::Message::Pong(_) => continue,
            _ => return Err(err("expected text frame")),
        };
        #[cfg(target_arch = "wasm32")]
        let text = match message {
            gloo_net::websocket::Message::Text(text) => text,
            _ => return Err(err("expected text frame")),
        };
        if text.len() > MAX_MESSAGE {
            return Err(err("message exceeds limit"));
        }
        return serde_json::from_str(&text).map_err(err);
    }
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
}
impl Client {
    pub async fn connect(url: &str, token: &str) -> Result<Self> {
        deadline(Duration::from_secs(10), async {
            #[cfg(not(target_arch = "wasm32"))]
            let mut socket = async_tungstenite::smol::connect_async(url)
                .await
                .map_err(err)?
                .0;
            #[cfg(target_arch = "wasm32")]
            let mut socket = Socket::open(url).map_err(err)?;
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
                }),
                _ => Err(err("relay rejected protocol handshake")),
            }
        })
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
        // Set before the first await: cancellation never enables another transfer.
        self.usable = false;
        self.next_id += 1;
        let id = self.next_id;
        let reply = deadline(timeout + Duration::from_secs(2), async {
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
        })
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
