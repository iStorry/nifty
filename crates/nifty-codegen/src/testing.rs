//! Helpers for tests that link apps against the runtime library.

use std::env;
use std::path::PathBuf;

use crate::RUNTIME_LIBRARY;

/// The runtime library `cargo build` left in the target folder this test runs from:
/// tests run from `<target>/<profile>/deps`.
///
/// # Panics
/// When it isn't there, which means the workspace wasn't built before testing.
#[must_use]
pub fn runtime_library() -> PathBuf {
    let test = env::current_exe().expect("a test knows where it is");
    let profile = test
        .parent()
        .and_then(|deps| deps.parent())
        .expect("tests run from the target folder's deps folder");
    let library = profile.join(RUNTIME_LIBRARY);
    assert!(
        library.is_file(),
        "{} is missing; run `cargo build --workspace` before testing",
        library.display()
    );
    library
}
