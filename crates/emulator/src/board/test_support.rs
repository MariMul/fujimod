use std::{ffi::OsString, path::PathBuf};

pub(crate) fn unicorn_library_path() -> PathBuf {
    selected_unicorn_library(std::env::var_os("CAMERA_UNICORN_LIBRARY"))
}

fn selected_unicorn_library(path: Option<OsString>) -> PathBuf {
    path.map(PathBuf::from)
        .expect("CAMERA_UNICORN_LIBRARY must select the Unicorn library")
}

#[cfg(test)]
mod tests {
    use super::selected_unicorn_library;
    use std::{ffi::OsString, path::PathBuf};

    #[test]
    #[should_panic(expected = "CAMERA_UNICORN_LIBRARY must select the Unicorn library")]
    fn missing_unicorn_library_is_refused() {
        selected_unicorn_library(None);
    }

    #[test]
    fn selected_unicorn_library_path_is_preserved() {
        assert_eq!(
            selected_unicorn_library(Some(OsString::from("/explicit/libunicorn.so"))),
            PathBuf::from("/explicit/libunicorn.so")
        );
    }
}
