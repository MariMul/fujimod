//! Direct LCD and control transport for loopback webviews without WebRTC.
use axum::{
    Router,
    body::Bytes,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use fujimod_emulator_devices::graphics::scanout::Frame;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use tokio::sync::watch;

#[derive(Clone)]
struct LocalRoom {
    key: Arc<str>,
    events: mpsc::Sender<(String, Socket)>,
    next: Arc<AtomicU64>,
    frames: watch::Receiver<Option<Bytes>>,
}

pub(crate) struct Socket {
    pub(crate) requests: async_channel::Receiver<Vec<u8>>,
    pub(crate) replies: async_channel::Sender<String>,
}

pub(crate) fn routes(
    key: &str,
    events: mpsc::Sender<(String, Socket)>,
    frames: watch::Receiver<Option<Bytes>>,
) -> Router {
    Router::new()
        .route("/emulator/{key}", get(upgrade))
        .with_state(LocalRoom {
            key: Arc::from(key),
            events,
            next: Arc::new(AtomicU64::new(0)),
            frames,
        })
}

async fn upgrade(
    Path(key): Path<String>,
    State(room): State<LocalRoom>,
    socket: WebSocketUpgrade,
) -> Response {
    if key.as_str() != room.key.as_ref() {
        return StatusCode::NOT_FOUND.into_response();
    }
    socket
        .max_message_size(64 * 1024)
        .max_frame_size(64 * 1024)
        .on_upgrade(move |socket| serve(socket, room))
}

async fn serve(mut socket: WebSocket, room: LocalRoom) {
    let (incoming, requests) = async_channel::bounded(64);
    let (replies, outgoing) = async_channel::bounded(64);
    let id = format!("local:{}", room.next.fetch_add(1, Ordering::Relaxed));
    if room
        .events
        .send((id, Socket { requests, replies }))
        .is_err()
    {
        return;
    }
    let mut frames = room.frames;
    frames.mark_changed();
    loop {
        let sent = tokio::select! {
            received = socket.recv() => match received {
                Some(Ok(Message::Binary(bytes))) => incoming.try_send(bytes.to_vec()).is_ok(),
                Some(Ok(Message::Text(text))) => incoming.try_send(text.as_bytes().to_vec()).is_ok(),
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => true,
                _ => false,
            },
            reply = outgoing.recv() => match reply {
                Ok(reply) => socket.send(Message::Text(reply.into())).await.is_ok(),
                Err(_) => false,
            },
            changed = frames.changed() => {
                let frame = frames.borrow_and_update().clone();
                match (changed, frame) {
                    (Ok(()), Some(frame)) => socket.send(Message::Binary(frame)).await.is_ok(),
                    (Ok(()), None) => true,
                    (Err(_), _) => false,
                }
            },
        };
        if !sent {
            break;
        }
    }
}

pub(crate) fn packet(frame: &Frame, sequence: u64) -> Bytes {
    let mut bytes = Vec::with_capacity(16 + frame.rgba.len());
    bytes.extend(frame.width.to_be_bytes());
    bytes.extend(frame.height.to_be_bytes());
    bytes.extend(sequence.to_be_bytes());
    bytes.extend_from_slice(&frame.rgba);
    Bytes::from(bytes)
}
