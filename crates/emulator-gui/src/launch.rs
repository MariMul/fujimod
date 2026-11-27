//! Launch the desktop application as the frontend of a native session.
use std::process::{Child, Command};

/// Arguments that connect the desktop application to an existing session.
pub fn arguments(endpoint: &str) -> Vec<String> {
    vec!["--endpoint".into(), endpoint.into()]
}

/// The link a browser page at `page` opens to join the session at `endpoint`:
/// the page with the endpoint's ticket fragment.
pub fn page_link(page: &str, endpoint: &str) -> String {
    let fragment = endpoint.find('#').map_or("", |at| &endpoint[at..]);
    format!("{}{fragment}", page.split('#').next().unwrap_or(page))
}

/// The desktop application, or a browser page, showing a camera. The camera
/// runs while it is open; controllers joining and leaving the room do not end
/// the run. A page stays open until the process is stopped.
pub struct Window(Option<Child>);

impl Window {
    pub fn open(&mut self) -> bool {
        match &mut self.0 {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => true,
        }
    }
    pub fn close(self) {
        if let Some(mut child) = self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    pub fn wait(self) -> Result<(), String> {
        match self.0 {
            Some(mut child) => child.wait().map(drop).map_err(|error| error.to_string()),
            None => Ok(()),
        }
    }
}

/// Start the desktop application for a camera session. With
/// `FUJIMOD_EMULATOR_GUI_PAGE` set to the URL of the built window frontend,
/// a browser joins instead: the link is printed, and also handed to
/// `FUJIMOD_EMULATOR_GUI_OPEN` when that names a program that opens URLs.
/// This serves hosts whose webview has no WebRTC.
pub fn connect(endpoint: &str) -> Result<Window, String> {
    if let Some(page) = std::env::var_os("FUJIMOD_EMULATOR_GUI_PAGE") {
        let link = page_link(&page.to_string_lossy(), endpoint);
        eprintln!("open the camera in a browser: {link}");
        if let Some(opener) = std::env::var_os("FUJIMOD_EMULATOR_GUI_OPEN") {
            Command::new(&opener)
                .arg(&link)
                .spawn()
                .map_err(|error| format!("{}: {error}", opener.to_string_lossy()))?;
        }
        return Ok(Window(None));
    }
    let executable = std::env::var_os("FUJIMOD_EMULATOR_GUI")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("fujimod-emulator-gui"));
    Command::new(&executable)
        .args(arguments(endpoint))
        .spawn()
        .map(|child| Window(Some(child)))
        .map_err(|error| format!("{}: {error}", executable.display()))
}

#[cfg(test)]
mod tests {
    use super::{arguments, page_link};

    #[test]
    fn a_browser_page_receives_the_session_ticket_in_its_fragment() {
        let endpoint = "http://localhost/#?remotecontrol=abc";
        assert_eq!(
            page_link("http://localhost:8765/", endpoint),
            "http://localhost:8765/#?remotecontrol=abc"
        );
        assert_eq!(
            page_link("http://localhost:8765/index.html#old", endpoint),
            "http://localhost:8765/index.html#?remotecontrol=abc"
        );
    }

    #[test]
    fn the_desktop_receives_the_session_endpoint_as_its_own_option() {
        assert_eq!(
            arguments("ws://127.0.0.1:48123/emulator"),
            ["--endpoint", "ws://127.0.0.1:48123/emulator"]
        );
    }
}
