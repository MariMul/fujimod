use crate::{Error, Result};
use wasm_bindgen::prelude::*;
#[wasm_bindgen(module = "/web/peer.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name=connectPeer)]
    async fn connect_peer(url: &str) -> std::result::Result<JsValue, JsValue>;
    type Channel;
    #[wasm_bindgen(method, catch)]
    async fn send(this: &Channel, text: &str) -> std::result::Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch)]
    async fn receive(this: &Channel) -> std::result::Result<JsValue, JsValue>;
    #[wasm_bindgen(method)]
    fn close(this: &Channel);
}
fn error(value: JsValue) -> Error {
    Error(value.as_string().unwrap_or_else(|| format!("{value:?}")))
}
pub(crate) struct PeerSocket(Channel);
impl PeerSocket {
    pub async fn connect(url: &str) -> Result<Self> {
        Ok(Self(
            connect_peer(url).await.map_err(error)?.unchecked_into(),
        ))
    }
    pub async fn send(&mut self, text: &str) -> Result<()> {
        self.0.send(text).await.map_err(error)?;
        Ok(())
    }
    pub async fn receive(&mut self) -> Result<String> {
        self.0
            .receive()
            .await
            .map_err(error)?
            .as_string()
            .ok_or_else(|| Error("invalid peer message".into()))
    }
}
impl Drop for PeerSocket {
    fn drop(&mut self) {
        self.0.close();
    }
}
