use std::fs;
use std::path::PathBuf;

/// An empty folder for one test, named after it so tests running in parallel don't collide.
pub(crate) fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nf-cli-test-{name}"));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("remove stale scratch dir");
    }
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}
