//! Check a share link's WebRTC bridge without touching USB.
//!
//! `fujimod-share-probe URL` (or FUJIMOD_USB_PROXY_URL) prints each connection
//! step with elapsed time, then stops after the proxy handshake. It sends no
//! USB request, not even a device listing.
use std::time::Instant;

fn main() {
    let url = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("FUJIMOD_USB_PROXY_URL").ok())
        .expect("give the share link as an argument or FUJIMOD_USB_PROXY_URL");
    let started = Instant::now();
    let progress: fujimod_usb_proxy::Progress = std::sync::Arc::new(move |step: &str| {
        println!("{:6.2}s  {step}", started.elapsed().as_secs_f64());
    });
    match futures_lite::future::block_on(fujimod_usb_proxy::Client::connect_with_progress(
        &url, "", progress,
    )) {
        Ok(_) => println!("BRIDGE READY: WebRTC and proxy handshake work; no USB request sent"),
        Err(error) => {
            println!("BRIDGE FAILED: {error}");
            std::process::exit(1);
        }
    }
}
