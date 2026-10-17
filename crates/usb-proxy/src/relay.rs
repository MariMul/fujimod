//! Optional relay service. It routes opaque transfers and owns device leases.
use crate::{Error, Result, protocol::*};
use axum::{
    Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex, mpsc, oneshot};

type Work = (String, Operation, u64, oneshot::Sender<Completion>);
struct Exporter {
    devices: Vec<Device>,
    sender: mpsc::Sender<Work>,
}
struct Inner {
    exporters: HashMap<String, Exporter>,
    leases: HashMap<String, String>,
}
#[derive(Clone)]
struct Relay {
    origins: Arc<Vec<String>>,
    token: Arc<String>,
    inner: Arc<Mutex<Inner>>,
    counter: Arc<AtomicU64>,
}
fn failure(error: impl Into<String>) -> Completion {
    Completion {
        error: Some(error.into()),
        ..Completion::default()
    }
}
fn equal(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |v, (a, b)| v | (a ^ b)) == 0
}
/// Mount on loopback behind HTTPS/WSS (for example Tailscale Serve). The token
/// is required for both roles and is never placed in a URL query or server log.
pub fn router(token: String, origins: Vec<String>) -> Result<Router> {
    if token.len() < 32 {
        return Err(Error("relay token must have at least 32 characters".into()));
    }
    let relay = Relay {
        origins: Arc::new(origins),
        token: Arc::new(token),
        inner: Arc::new(Mutex::new(Inner {
            exporters: HashMap::new(),
            leases: HashMap::new(),
        })),
        counter: Arc::new(AtomicU64::new(1)),
    };
    Ok(Router::new()
        .route(
            "/usb-proxy/",
            get(|| async { Html(include_str!("../web/index.html")) }),
        )
        .route(
            "/usb-proxy/exporter.js",
            get(|| async {
                (
                    [("content-type", "text/javascript")],
                    include_str!("../web/exporter.js"),
                )
            }),
        )
        .route(
            "/usb-proxy/usb-exporter.js",
            get(|| async {
                (
                    [("content-type", "text/javascript")],
                    include_str!("../web/usb-exporter.js"),
                )
            }),
        )
        .route("/usb-proxy/ws", get(upgrade))
        .with_state(relay)
        .merge(crate::signaling::router()))
}
async fn upgrade(
    State(relay): State<Relay>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    // Native clients omit Origin; browser clients need an allowed origin.
    if let Some(origin) = headers.get("origin") {
        let origin = origin.to_str().unwrap_or("");
        let host = headers
            .get("host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if origin != format!("https://{host}")
            && origin != format!("http://{host}")
            && !relay.origins.iter().any(|allowed| allowed == origin)
        {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    ws.max_message_size(MAX_MESSAGE)
        .max_frame_size(MAX_MESSAGE)
        .on_upgrade(|socket| session(relay, socket))
}
async fn receive(socket: &mut WebSocket) -> Option<Frame> {
    loop {
        match socket.recv().await? {
            Ok(Message::Text(text)) => return serde_json::from_str(&text).ok(),
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
            _ => return None,
        }
    }
}
async fn send(socket: &mut WebSocket, frame: &Frame) -> bool {
    socket
        .send(Message::Text(serde_json::to_string(frame).unwrap().into()))
        .await
        .is_ok()
}
async fn session(relay: Relay, mut socket: WebSocket) {
    let Ok(Some(Frame::Hello {
        version: VERSION,
        role,
        token,
    })) = tokio::time::timeout(Duration::from_secs(10), receive(&mut socket)).await
    else {
        return;
    };
    if !equal(&token, &relay.token) || !["exporter", "controller"].contains(&role.as_str()) {
        return;
    }
    let id = relay.counter.fetch_add(1, Ordering::Relaxed).to_string();
    if !send(
        &mut socket,
        &Frame::Ready {
            version: VERSION,
            id: id.clone(),
        },
    )
    .await
    {
        return;
    }
    if role == "exporter" {
        exporter(relay, socket, id).await;
    } else {
        controller(relay, socket, id).await;
    }
}
async fn exporter(relay: Relay, socket: WebSocket, id: String) {
    let (tx, mut rx) = mpsc::channel::<Work>(64);
    relay.inner.lock().await.exporters.insert(
        id.clone(),
        Exporter {
            devices: vec![],
            sender: tx,
        },
    );
    let (mut output, mut input) = socket.split();
    let mut pending = HashMap::new();
    let mut sequence = 0;
    loop {
        tokio::select! {
            work = rx.recv() => {
                let Some((device, operation, timeout_ms, reply)) = work else { break };
                if reply.is_closed() { continue; }
                pending.retain(|_, sender: &mut oneshot::Sender<Completion>| !sender.is_closed());
                if pending.len() >= 128 { let _ = reply.send(failure("exporter request limit")); continue; }
                sequence += 1;
                let frame = Frame::Request { id: sequence, device, operation, timeout_ms };
                if output.send(Message::Text(serde_json::to_string(&frame).unwrap().into())).await.is_err() { break; }
                pending.insert(sequence, reply);
            }
            frame = input.next() => {
                match frame {
                    Some(Ok(Message::Text(text))) => match serde_json::from_str::<Frame>(&text) {
                        Ok(Frame::Inventory { mut devices }) if devices.len() <= 128 => {
                            // Never allow a browser to overwrite another exporter's namespace.
                            if devices.iter().any(|d| d.id.is_empty() || d.id.contains('/') || d.id.len() > 128) { break; }
                            for device in &mut devices { device.id = format!("{id}/{}", device.id); }
                            if let Some(peer) = relay.inner.lock().await.exporters.get_mut(&id) { peer.devices = devices; }
                        }
                        Ok(Frame::Reply { id, completion }) => { if let Some(reply) = pending.remove(&id) { let _ = reply.send(completion); } }
                        _ => break,
                    },
                    Some(Ok(Message::Ping(bytes))) => { if output.send(Message::Pong(bytes)).await.is_err() { break; } }
                    Some(Ok(Message::Pong(_))) => (),
                    _ => break,
                }
            }
        }
    }
    let mut inner = relay.inner.lock().await;
    inner.exporters.remove(&id);
    inner
        .leases
        .retain(|device, _| !device.starts_with(&format!("{id}/")));
    // Dropping pending senders wakes controllers, never fabricates USB success.
}
async fn forward(relay: &Relay, device: &str, operation: Operation, timeout_ms: u64) -> Completion {
    let Some((exporter, local)) = device.split_once('/') else {
        return failure("invalid device route");
    };
    let sender = relay
        .inner
        .lock()
        .await
        .exporters
        .get(exporter)
        .map(|p| p.sender.clone());
    let Some(sender) = sender else {
        return failure("exporter disconnected; completion unknown");
    };
    let (tx, rx) = oneshot::channel();
    match tokio::time::timeout(Duration::from_millis(timeout_ms), async {
        sender
            .send((local.into(), operation, timeout_ms, tx))
            .await
            .map_err(|_| ())?;
        rx.await.map_err(|_| ())
    })
    .await
    {
        Ok(Ok(result)) => result,
        _ => failure("exporter disconnected or deadline expired; completion unknown"),
    }
}
async fn controller(relay: Relay, mut socket: WebSocket, owner: String) {
    while let Some(Frame::Request {
        id,
        device,
        operation,
        timeout_ms,
    }) = receive(&mut socket).await
    {
        if !(1..=120000).contains(&timeout_ms) {
            break;
        }
        let completion = if let Err(error) = operation.validate() {
            failure(error.to_string())
        } else if matches!(operation, Operation::List) {
            Completion {
                devices: relay
                    .inner
                    .lock()
                    .await
                    .exporters
                    .values()
                    .flat_map(|p| p.devices.clone())
                    .collect(),
                ..Completion::default()
            }
        } else {
            let allowed = {
                let mut inner = relay.inner.lock().await;
                let exists = inner
                    .exporters
                    .values()
                    .any(|p| p.devices.iter().any(|d| d.id == device));
                if matches!(operation, Operation::Open { .. }) {
                    if exists && !inner.leases.contains_key(&device) {
                        inner.leases.insert(device.clone(), owner.clone());
                        true
                    } else {
                        false
                    }
                } else {
                    inner.leases.get(&device) == Some(&owner)
                }
            };
            if !allowed {
                failure("device unavailable or leased by another controller")
            } else {
                let closing = matches!(operation, Operation::Close);
                let completion = forward(&relay, &device, operation, timeout_ms).await;
                if closing && completion.error.is_none() {
                    relay.inner.lock().await.leases.remove(&device);
                }
                completion
            }
        };
        let failed = completion.error.is_some();
        if !send(&mut socket, &Frame::Reply { id, completion }).await || failed {
            break;
        }
    }
    // Release host claims only, never send camera protocol cleanup. Retain the
    // lease after an uncertain close until the browser exporter disconnects.
    let owned: Vec<_> = relay
        .inner
        .lock()
        .await
        .leases
        .iter()
        .filter(|(_, v)| *v == &owner)
        .map(|(k, _)| k.clone())
        .collect();
    for device in owned {
        if forward(&relay, &device, Operation::Close, 5000)
            .await
            .error
            .is_none()
        {
            relay.inner.lock().await.leases.remove(&device);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Client;
    use async_tungstenite::{smol::connect_async, tungstenite::Message as Wire};
    const TOKEN: &str = "0123456789abcdef0123456789abcdef";
    async fn server() -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/usb-proxy/ws", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, router(TOKEN.into(), vec![]).unwrap())
                .await
                .unwrap();
        });
        (url, server)
    }
    async fn fixture(
        url: &str,
    ) -> (
        String,
        tokio::task::JoinHandle<()>,
        mpsc::Receiver<Operation>,
    ) {
        let (mut socket, _) = connect_async(url).await.unwrap();
        socket
            .send(Wire::Text(
                serde_json::to_string(&Frame::Hello {
                    version: VERSION,
                    role: "exporter".into(),
                    token: TOKEN.into(),
                })
                .unwrap()
                .into(),
            ))
            .await
            .unwrap();
        let ready: Frame =
            serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        let Frame::Ready { id, .. } = ready else {
            panic!()
        };
        socket
            .send(Wire::Text(
                serde_json::to_string(&Frame::Inventory {
                    devices: vec![Device {
                        id: "camera".into(),
                        vendor_id: 0x04cb,
                        product_id: 0xff80,
                        manufacturer: None,
                        product: None,
                        serial: None,
                        interfaces: vec![],
                    }],
                })
                .unwrap()
                .into(),
            ))
            .await
            .unwrap();
        let (tx, rx) = mpsc::channel(16);
        let task = tokio::spawn(async move {
            while let Some(Ok(frame)) = socket.next().await {
                let Ok(Frame::Request { id, operation, .. }) =
                    serde_json::from_str(frame.to_text().unwrap_or(""))
                else {
                    continue;
                };
                tx.send(operation.clone()).await.unwrap();
                let completion = match operation {
                    Operation::TransferIn { length: 7, .. } => Completion {
                        data: "AQID".into(),
                        transferred: 3,
                        error: Some("stall".into()),
                        ..Completion::default()
                    },
                    Operation::TransferIn { length: 9, .. } => {
                        std::future::pending::<()>().await;
                        unreachable!()
                    }
                    _ => Completion::default(),
                };
                if socket
                    .send(Wire::Text(
                        serde_json::to_string(&Frame::Reply { id, completion })
                            .unwrap()
                            .into(),
                    ))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
        (format!("{id}/camera"), task, rx)
    }
    async fn available(url: &str) -> Client {
        let mut client = Client::connect(url, TOKEN).await.unwrap();
        for _ in 0..100 {
            if !client.devices().await.unwrap().is_empty() {
                return client;
            }
            tokio::task::yield_now().await;
        }
        panic!("inventory not published")
    }
    fn open() -> Operation {
        Operation::Open {
            configuration: 1,
            interface: 2,
            alternate: 3,
        }
    }
    #[tokio::test]
    async fn authentication_and_version_are_required() {
        let (url, server) = server().await;
        assert!(Client::connect(&url, "wrong").await.is_err());
        let (mut socket, _) = connect_async(&url).await.unwrap();
        socket
            .send(Wire::Text(
                serde_json::to_string(&Frame::Hello {
                    version: 99,
                    role: "controller".into(),
                    token: TOKEN.into(),
                })
                .unwrap()
                .into(),
            ))
            .await
            .unwrap();
        assert!(!matches!(socket.next().await, Some(Ok(Wire::Text(_)))));
        server.abort();
    }
    #[tokio::test]
    async fn leases_exclude_other_controllers_and_preserve_partial_failure() {
        let (url, server) = server().await;
        let (route, exporter, mut events) = fixture(&url).await;
        let mut first = available(&url).await;
        let mut second = Client::connect(&url, TOKEN).await.unwrap();
        assert!(
            first
                .request(&route, open(), Duration::from_secs(1))
                .await
                .unwrap()
                .error
                .is_none()
        );
        assert!(matches!(events.recv().await, Some(Operation::Open { .. })));
        assert!(
            second
                .request(&route, open(), Duration::from_secs(1))
                .await
                .unwrap()
                .error
                .is_some()
        );
        assert!(events.try_recv().is_err());
        let partial = first
            .request(
                &route,
                Operation::TransferIn {
                    endpoint: 0x82,
                    length: 7,
                },
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        assert_eq!(partial.error.as_deref(), Some("stall"));
        assert_eq!(partial.data, "AQID");
        assert_eq!(partial.transferred, 3);
        assert!(
            first
                .request(&route, Operation::Close, Duration::from_secs(1))
                .await
                .is_err()
        );
        assert!(matches!(
            events.recv().await,
            Some(Operation::TransferIn { .. })
        ));
        assert!(matches!(events.recv().await, Some(Operation::Close)));
        exporter.abort();
        server.abort();
    }
    #[tokio::test]
    async fn deadline_fences_without_replaying_transfer() {
        let (url, server) = server().await;
        let (route, exporter, mut events) = fixture(&url).await;
        let mut client = available(&url).await;
        client
            .request(&route, open(), Duration::from_secs(1))
            .await
            .unwrap();
        events.recv().await;
        let result = client
            .request(
                &route,
                Operation::TransferIn {
                    endpoint: 0x82,
                    length: 9,
                },
                Duration::from_millis(30),
            )
            .await
            .unwrap();
        assert!(result.error.unwrap().contains("completion unknown"));
        assert!(matches!(
            events.recv().await,
            Some(Operation::TransferIn { length: 9, .. })
        ));
        assert!(
            client
                .request(&route, Operation::Close, Duration::from_secs(1))
                .await
                .is_err()
        );
        assert!(events.try_recv().is_err());
        exporter.abort();
        server.abort();
    }
}
