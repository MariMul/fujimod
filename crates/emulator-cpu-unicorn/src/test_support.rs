//! Explicit native execution input for source-authored instruction tests.
use crate::Library;

pub(crate) fn library() -> Library {
    let path = std::env::var_os("CAMERA_UNICORN_LIBRARY")
        .expect("CAMERA_UNICORN_LIBRARY must select the counted Unicorn library");
    Library::load(std::path::Path::new(&path)).expect("load the selected Unicorn library")
}
