//! Windows apps are folders holding the executable, named after the app.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::CliError;

/// Writes `<output>/<name>/<name>.exe` from `executable`, replacing an earlier build, and
/// returns the folder.
pub(crate) fn write_app(output: &Path, name: &str, executable: &Path) -> Result<PathBuf, CliError> {
    let app = output.join(name);
    let failed =
        |error: io::Error| CliError::new(format!("can't write {}: {error}", app.display()));
    if app.exists() {
        fs::remove_dir_all(&app).map_err(failed)?;
    }
    fs::create_dir_all(&app).map_err(failed)?;
    fs::copy(executable, app.join(format!("{name}.exe"))).map_err(failed)?;
    Ok(app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::scratch_dir;

    #[test]
    fn writes_a_folder_holding_the_executable() {
        let dir = scratch_dir("build-windows-app");
        let executable = dir.join("app.exe");
        fs::write(&executable, "machine code").expect("write a stand-in executable");

        let app = write_app(&dir.join("build"), "todos", &executable).expect("the app is written");

        assert_eq!(app, dir.join("build/todos"));
        let bundled = app.join("todos.exe");
        assert_eq!(
            fs::read_to_string(bundled).expect("executable"),
            "machine code"
        );
    }

    #[test]
    fn building_again_replaces_the_previous_app() {
        let dir = scratch_dir("build-windows-rebuild");
        let executable = dir.join("app.exe");
        fs::write(&executable, "machine code").expect("write a stand-in executable");
        let app = write_app(&dir, "app", &executable).expect("first build");
        fs::write(app.join("stale.txt"), "").expect("write a stale file");

        write_app(&dir, "app", &executable).expect("second build");

        assert!(!app.join("stale.txt").exists());
    }

    #[test]
    fn names_with_dots_keep_their_whole_name() {
        let dir = scratch_dir("build-windows-dotted");
        let executable = dir.join("app.exe");
        fs::write(&executable, "machine code").expect("write a stand-in executable");

        let app = write_app(&dir, "todo.v2", &executable).expect("the app is written");

        assert!(app.join("todo.v2.exe").is_file());
    }
}
