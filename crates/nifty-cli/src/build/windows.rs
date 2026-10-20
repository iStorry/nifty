//! Windows apps are folders: the runner, renamed after the app, with the project beside it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nifty_syntax::project::ENTRY_FILE;

use super::write_project;
use crate::error::CliError;
use crate::frontend::SourceFile;

/// The project sits in its own folder, so a runner that happens to share a folder with a
/// `main.nf`, such as one installed next to `nf`, doesn't mistake it for a bundled app.
const PROJECT_DIR: &str = "app";

/// Writes `<output>/<name>/<name>.exe`, replacing an earlier build, and returns the folder.
pub(crate) fn write_app(
    output: &Path,
    name: &str,
    files: &[SourceFile],
    runner: &Path,
) -> Result<PathBuf, CliError> {
    let app = output.join(name);
    let failed =
        |error: io::Error| CliError::new(format!("can't write {}: {error}", app.display()));
    if app.exists() {
        fs::remove_dir_all(&app).map_err(failed)?;
    }
    let project = app.join(PROJECT_DIR);
    fs::create_dir_all(&project).map_err(failed)?;
    fs::copy(runner, app.join(format!("{name}.exe"))).map_err(failed)?;
    write_project(&project, files).map_err(failed)?;
    Ok(app)
}

/// The project inside the app whose executable is `executable`, if it is in one.
pub(crate) fn bundled_project(executable: &Path) -> Option<PathBuf> {
    let project = executable.parent()?.join(PROJECT_DIR);
    project.join(ENTRY_FILE).is_file().then_some(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::sample_project;
    use crate::test_support::scratch_dir;

    #[test]
    fn writes_an_app_the_runner_can_find_its_project_in() {
        let dir = scratch_dir("build-windows-app");
        let runner = dir.join("nf-runner.exe");
        fs::write(&runner, "runner").expect("write a stand-in runner");

        let app = write_app(
            &dir.join("build"),
            "todos",
            &sample_project("window(\"T\") {}"),
            &runner,
        )
        .expect("the app is written");

        assert_eq!(app, dir.join("build/todos"));
        let executable = app.join("todos.exe");
        assert_eq!(fs::read_to_string(&executable).expect("runner"), "runner");
        let project = bundled_project(&executable).expect("the project is bundled");
        assert_eq!(
            fs::read_to_string(project.join(ENTRY_FILE)).expect("source"),
            "window(\"T\") {}"
        );
        assert!(project.join("ui/header.nf").is_file());
    }

    #[test]
    fn building_again_replaces_the_previous_app() {
        let dir = scratch_dir("build-windows-rebuild");
        let runner = dir.join("nf-runner.exe");
        fs::write(&runner, "runner").expect("write a stand-in runner");
        let app = write_app(&dir, "app", &sample_project("old"), &runner).expect("first build");
        fs::write(app.join(PROJECT_DIR).join("stale.nf"), "").expect("write a stale file");

        write_app(&dir, "app", &sample_project("new"), &runner).expect("second build");

        assert!(!app.join(PROJECT_DIR).join("stale.nf").exists());
    }

    #[test]
    fn names_with_dots_keep_their_whole_name() {
        let dir = scratch_dir("build-windows-dotted");
        let runner = dir.join("nf-runner.exe");
        fs::write(&runner, "runner").expect("write a stand-in runner");

        let app =
            write_app(&dir, "todo.v2", &sample_project(""), &runner).expect("the app is written");

        assert!(app.join("todo.v2.exe").is_file());
    }

    #[test]
    fn a_runner_outside_an_app_has_no_project() {
        let dir = scratch_dir("build-windows-loose-runner");
        fs::write(dir.join(ENTRY_FILE), "").expect("a project next to the runner");
        assert_eq!(bundled_project(&dir.join("nf-runner.exe")), None);
    }
}
