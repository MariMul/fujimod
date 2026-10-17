//! Portable capability ticket embedded only in the URL fragment.
use crate::{Error, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ticket {
    pub v: u32,
    pub signal: String,
    pub room: String,
    pub key: String,
    #[serde(default)]
    pub stun: Vec<String>,
}
impl Ticket {
    pub fn from_url(value: &str) -> Result<Option<Self>> {
        if !value.contains('#') {
            return Ok(None);
        }
        let url = url::Url::parse(value).map_err(|_| Error("invalid share URL".into()))?;
        let Some(fragment) = url.fragment() else {
            return Ok(None);
        };
        let values: Vec<_> =
            url::form_urlencoded::parse(fragment.trim_start_matches('?').as_bytes())
                .filter(|(key, _)| key == "remotecontrol")
                .collect();
        if values.is_empty() {
            return Ok(None);
        }
        if values.len() != 1 || values[0].1.len() > 4096 {
            return Err(Error("invalid share ticket".into()));
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(values[0].1.as_bytes())
            .map_err(|_| Error("invalid share ticket".into()))?;
        let ticket: Self =
            serde_json::from_slice(&bytes).map_err(|_| Error("invalid share ticket".into()))?;
        let signal =
            url::Url::parse(&ticket.signal).map_err(|_| Error("invalid signaling URL".into()))?;
        if ticket.v != 1
            || !["ws", "wss"].contains(&signal.scheme())
            || !signal.username().is_empty()
            || signal.password().is_some()
            || URL_SAFE_NO_PAD.decode(&ticket.room).map(|v| v.len()).ok() != Some(32)
            || URL_SAFE_NO_PAD.decode(&ticket.key).map(|v| v.len()).ok() != Some(32)
            || ticket.stun.len() > 4
            || ticket
                .stun
                .iter()
                .any(|s| !s.starts_with("stun:") || s.len() > 256)
        {
            return Err(Error("unsupported share ticket".into()));
        }
        Ok(Some(ticket))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_browser_url_is_a_native_controller_capability() {
        let ticket = Ticket {
            v: 1,
            signal: "wss://signal.example/usb-proxy/signal".into(),
            room: URL_SAFE_NO_PAD.encode([1; 32]),
            key: URL_SAFE_NO_PAD.encode([2; 32]),
            stun: vec!["stun:stun.cloudflare.com:3478".into()],
        };
        let link = format!(
            "https://camera.example/dev-tools/?firmware=body#?remotecontrol={}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&ticket).unwrap())
        );
        let parsed = Ticket::from_url(&link).unwrap().unwrap();
        assert_eq!(parsed.signal, ticket.signal);
        assert_eq!(parsed.key, ticket.key);
        assert!(
            Ticket::from_url("wss://camera.example/usb-proxy/ws")
                .unwrap()
                .is_none()
        );
        assert!(Ticket::from_url("https://camera.example/#?remotecontrol=e30").is_err());
        assert!(Ticket::from_url(&(link.clone() + "&remotecontrol=e30")).is_err());
    }
}
