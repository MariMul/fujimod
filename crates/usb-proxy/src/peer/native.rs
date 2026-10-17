//! str0m is driven by one owned I/O task. It works under native Python, Tokio,
//! smol or a plain block_on; no caller runtime or callbacks are required.
use crate::{Error, Result, protocol::MAX_MESSAGE, share::Ticket};
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use async_channel::{Receiver, Sender};
use async_tungstenite::{
    WebSocketStream,
    smol::{ConnectStream, connect_async},
    tungstenite::Message,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::StreamExt;
use rand::Rng;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    net::{SocketAddr, ToSocketAddrs},
    time::{Duration, Instant},
};
use str0m::{
    Candidate, Event, Input, Output, Rtc,
    change::SdpAnswer,
    channel::ChannelId,
    net::{Protocol, Receive},
};
fn error(e: impl std::fmt::Display) -> Error {
    Error(e.to_string())
}
type Socket = WebSocketStream<ConnectStream>;
async fn write(socket: &mut Socket, value: Value) -> Result<()> {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .map_err(error)
}
async fn read(socket: &mut Socket) -> Result<Value> {
    loop {
        match socket
            .next()
            .await
            .ok_or_else(|| error("share unavailable"))?
            .map_err(error)?
        {
            Message::Text(text) if text.len() <= 65536 => {
                return serde_json::from_str(&text).map_err(error);
            }
            Message::Ping(_) | Message::Pong(_) => (),
            _ => return Err(error("signaling disconnected")),
        }
    }
}
fn cipher(ticket: &Ticket) -> Result<Aes256Gcm> {
    Aes256Gcm::new_from_slice(&URL_SAFE_NO_PAD.decode(&ticket.key).map_err(error)?).map_err(error)
}
fn seal(ticket: &Ticket, peer: &str, value: Value) -> Result<String> {
    let iv: [u8; 12] = rand::rng().random();
    let aad = format!("{}/{peer}", ticket.room);
    let encrypted = cipher(ticket)?
        .encrypt(
            Nonce::from_slice(&iv),
            Payload {
                msg: value.to_string().as_bytes(),
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| error("signaling encryption failed"))?;
    let mut data = iv.to_vec();
    data.extend(encrypted);
    Ok(URL_SAFE_NO_PAD.encode(data))
}
fn unseal(ticket: &Ticket, peer: &str, value: &str) -> Result<Value> {
    let bytes = URL_SAFE_NO_PAD.decode(value).map_err(error)?;
    if bytes.len() < 28 {
        return Err(error("invalid encrypted signaling"));
    }
    let aad = format!("{}/{peer}", ticket.room);
    let decoded = cipher(ticket)?
        .decrypt(
            Nonce::from_slice(&bytes[..12]),
            Payload {
                msg: &bytes[12..],
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| error("signaling authentication failed"))?;
    serde_json::from_slice(&decoded).map_err(error)
}
pub(crate) struct PeerSocket {
    send: Sender<String>,
    receive: Receiver<Result<String>>,
}
impl PeerSocket {
    pub async fn connect(url: &str) -> Result<Self> {
        let ticket = Ticket::from_url(url)?.ok_or_else(|| error("invalid share URL"))?;
        let (tx, outbound) = async_channel::bounded(1);
        let (incoming, rx) = async_channel::bounded(16);
        let (ready, wait) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("fujimod-webrtc".into())
            .spawn(move || {
                futures_lite::future::block_on(async {
                    if let Err(error) = run(ticket, outbound, incoming.clone(), ready.clone()).await
                    {
                        let _ = ready.send(Err(error.to_string())).await;
                        let _ = incoming.send(Err(error)).await;
                    }
                });
            })
            .map_err(error)?;
        wait.recv().await.map_err(error)?.map_err(error)?;
        Ok(Self {
            send: tx,
            receive: rx,
        })
    }
    pub async fn send(&mut self, text: &str) -> Result<()> {
        self.send.send(text.into()).await.map_err(error)
    }
    pub async fn receive(&mut self) -> Result<String> {
        self.receive
            .recv()
            .await
            .map_err(|_| error("peer disconnected; completion unknown"))?
    }
}
impl Drop for PeerSocket {
    fn drop(&mut self) {
        self.send.close();
        self.receive.close();
    }
}

fn drain_setup(rtc: &mut Rtc) -> Result<()> {
    loop {
        if let Output::Timeout(_) = rtc.poll_output().map_err(error)? {
            return Ok(());
        }
    }
}
async fn run(
    ticket: Ticket,
    outbound: Receiver<String>,
    incoming: Sender<Result<String>>,
    ready: Sender<std::result::Result<(), String>>,
) -> Result<()> {
    let std_socket = std::net::UdpSocket::bind("0.0.0.0:0").map_err(error)?;
    let port = std_socket.local_addr().map_err(error)?.port();
    let mut rtc = Rtc::new(Instant::now());
    let mut local = Vec::new();
    for iface in if_addrs::get_if_addrs().map_err(error)? {
        if iface.ip().is_ipv4() {
            local.push(SocketAddr::new(iface.ip(), port));
        }
    }
    local.sort();
    local.dedup();
    for addr in &local {
        rtc.add_local_candidate(Candidate::host(*addr, "udp").map_err(error)?);
        drain_setup(&mut rtc)?;
    }
    // An optional STUN lookup uses the same socket as ICE, without a TURN path.
    for server in &ticket.stun {
        let name = server.trim_start_matches("stun:");
        let name = if name.contains(':') {
            name.to_owned()
        } else {
            format!("{name}:3478")
        };
        if let Ok(addresses) = name.to_socket_addrs() {
            if let Some(server) = addresses.into_iter().find(SocketAddr::is_ipv4) {
                let mut client = stunclient::StunClient::new(server);
                client.set_timeout(Duration::from_secs(2));
                if let Ok(external) = client.query_external_address(&std_socket) {
                    let route = std::net::UdpSocket::bind("0.0.0.0:0").map_err(error)?;
                    route.connect(server).map_err(error)?;
                    let base = SocketAddr::new(route.local_addr().map_err(error)?.ip(), port);
                    rtc.add_local_candidate(
                        Candidate::server_reflexive(external, base, "udp").map_err(error)?,
                    );
                    drain_setup(&mut rtc)?;
                }
            }
        }
    }
    std_socket.set_nonblocking(true).map_err(error)?;
    let udp = async_net::UdpSocket::from(async_io::Async::new(std_socket).map_err(error)?);
    let (mut signal, _) = connect_async(&ticket.signal).await.map_err(error)?;
    write(
        &mut signal,
        json!({"type":"join","role":"client","room":ticket.room}),
    )
    .await?;
    let welcome = read(&mut signal).await?;
    let peer = welcome["peer"]
        .as_str()
        .filter(|_| welcome["type"] == "ready")
        .ok_or_else(|| error("share unavailable"))?
        .to_owned();
    let mut change = rtc.sdp_api();
    change.add_channel("fujimod-usb".into());
    let (offer, pending) = change
        .apply()
        .ok_or_else(|| error("missing WebRTC offer"))?;
    drain_setup(&mut rtc)?;
    write(&mut signal, json!({"type":"signal","data":seal(&ticket,&peer,json!({"type":"offer","sdp":offer.to_sdp_string()}))?})).await?;
    let answer = read(&mut signal).await?;
    if answer["type"] != "signal" || answer["from"] != "host" {
        return Err(error("share ended"));
    }
    let answer = unseal(
        &ticket,
        &peer,
        answer["data"]
            .as_str()
            .ok_or_else(|| error("invalid signaling reply"))?,
    )?;
    if answer["type"] != "answer" {
        return Err(error("invalid WebRTC answer"));
    }
    rtc.sdp_api()
        .accept_answer(
            pending,
            SdpAnswer::from_sdp_string(answer["sdp"].as_str().ok_or_else(|| error("missing SDP"))?)
                .map_err(error)?,
        )
        .map_err(error)?;
    let mut channel: Option<ChannelId> = None;
    let mut assembled = Vec::new();
    let mut packets: VecDeque<Vec<u8>> = VecDeque::new();
    let mut buffer = vec![0u8; 65536];
    let mut writable = true;
    let mut signal_alive = true;
    let started = Instant::now();
    loop {
        let timeout = loop {
            match rtc.poll_output().map_err(error)? {
                Output::Timeout(at) => break at,
                Output::Transmit(packet) => {
                    udp.send_to(&packet.contents, packet.destination)
                        .await
                        .map_err(error)?;
                }
                Output::Event(event) => match event {
                    Event::ChannelOpen(id, label) if label == "fujimod-usb" => {
                        channel = Some(id);
                        if ready.send(Ok(())).await.is_err() {
                            return Ok(());
                        }
                    }
                    Event::ChannelData(data) => {
                        if Some(data.id) != channel
                            || !data.binary
                            || data.data.is_empty()
                            || data.data[0] > 1
                            || assembled.len() + data.data.len() - 1 > MAX_MESSAGE
                        {
                            return Err(error("invalid peer frame"));
                        }
                        assembled.extend_from_slice(&data.data[1..]);
                        if data.data[0] == 1 {
                            incoming
                                .send(Ok(String::from_utf8(std::mem::take(&mut assembled))
                                    .map_err(error)?))
                                .await
                                .map_err(error)?;
                        }
                    }
                    Event::ChannelClose(_) => {
                        return Err(error("peer disconnected; completion unknown"));
                    }
                    _ => (),
                },
            }
        };
        if !rtc.is_alive() || (channel.is_none() && started.elapsed() > Duration::from_secs(20)) {
            return Err(error("direct WebRTC connection unavailable"));
        }
        if writable {
            if let (Some(id), Some(packet)) = (channel, packets.front()) {
                let accepted = rtc
                    .channel(id)
                    .ok_or_else(|| error("data channel closed"))?
                    .write(true, packet)
                    .map_err(error)?;
                if accepted {
                    packets.pop_front();
                } else {
                    writable = false;
                }
                continue; // Drain the complete RTC output after every mutation.
            }
        }
        enum Wake {
            Out(std::result::Result<String, async_channel::RecvError>),
            Udp(std::io::Result<(usize, SocketAddr)>),
            Tick,
            Signal(Result<Value>),
        }
        let wake = futures_lite::future::or(
            async { Wake::Out(outbound.recv().await) },
            futures_lite::future::or(
                async { Wake::Udp(udp.recv_from(&mut buffer).await) },
                futures_lite::future::or(
                    async {
                        futures_timer::Delay::new(
                            timeout.saturating_duration_since(Instant::now()),
                        )
                        .await;
                        Wake::Tick
                    },
                    async {
                        if !signal_alive {
                            std::future::pending::<()>().await;
                        }
                        Wake::Signal(read(&mut signal).await)
                    },
                ),
            ),
        )
        .await;
        match wake {
            Wake::Out(Ok(text)) => {
                if text.len() > MAX_MESSAGE {
                    return Err(error("USB message exceeds limit"));
                }
                for (index, chunk) in text.as_bytes().chunks(16000).enumerate() {
                    let mut packet = vec![u8::from((index + 1) * 16000 >= text.len())];
                    packet.extend_from_slice(chunk);
                    packets.push_back(packet);
                }
            }
            Wake::Out(Err(_)) => return Ok(()),
            Wake::Udp(result) => {
                let (size, source) = result.map_err(error)?;
                let route = std::net::UdpSocket::bind("0.0.0.0:0").map_err(error)?;
                route.connect(source).map_err(error)?;
                let destination = SocketAddr::new(route.local_addr().map_err(error)?.ip(), port);
                writable = true;
                if let Ok(receive) =
                    Receive::new(Protocol::Udp, source, destination, &buffer[..size])
                {
                    rtc.handle_input(Input::Receive(Instant::now(), receive))
                        .map_err(error)?;
                }
            }
            Wake::Tick => {
                rtc.handle_input(Input::Timeout(Instant::now()))
                    .map_err(error)?;
                writable = true;
            }
            Wake::Signal(Ok(frame)) if frame["type"] == "ended" => {
                return Err(error("camera sharing ended"));
            }
            Wake::Signal(Err(_)) => signal_alive = false,
            Wake::Signal(Ok(_)) => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signaling_is_authenticated_and_bound_to_the_room_and_peer() {
        let ticket = Ticket {
            v: 1,
            signal: "wss://example.test/signal".into(),
            room: URL_SAFE_NO_PAD.encode([1; 32]),
            key: URL_SAFE_NO_PAD.encode([2; 32]),
            stun: vec![],
        };
        let text = seal(&ticket, "7", json!({"type":"offer","sdp":"private SDP"})).unwrap();
        assert!(!text.contains("private"));
        assert_eq!(unseal(&ticket, "7", &text).unwrap()["sdp"], "private SDP");
        assert!(unseal(&ticket, "8", &text).is_err());
        let mut other = ticket.clone();
        other.room = URL_SAFE_NO_PAD.encode([3; 32]);
        assert!(unseal(&other, "7", &text).is_err());
        other = ticket.clone();
        other.key = URL_SAFE_NO_PAD.encode([4; 32]);
        assert!(unseal(&other, "7", &text).is_err());
    }
}
