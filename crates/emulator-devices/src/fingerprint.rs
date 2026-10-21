//! Content identities for observational machine comparison. No host address,
//! clock, allocation identity or mutable borrowed bytes enter these digests.
use sha2::{Digest, Sha256};
pub fn bytes(bytes: &[u8]) -> serde_json::Value {
    serde_json::json!({"bytes":bytes.len(), "sha256":format!("{:x}", Sha256::digest(bytes))})
}

/// Hash sensor words in canonical little-endian order with bounded scratch.
pub fn words16(words: &[u16]) -> serde_json::Value {
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 4096];
    for chunk in words.chunks(bytes.len() / 2) {
        for (word, pair) in chunk.iter().zip(bytes.chunks_exact_mut(2)) {
            pair.copy_from_slice(&word.to_le_bytes());
        }
        hash.update(&bytes[..chunk.len() * 2]);
    }
    serde_json::json!({"words":words.len(),"sha256":format!("{:x}",hash.finalize())})
}
