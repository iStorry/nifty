//! macOS apps are bundles: a folder ending in `.app` holding the runner as the executable
//! and the project as a resource.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nifty_syntax::project::ENTRY_FILE;

use super::write_project;
use crate::error::CliError;
use crate::frontend::SourceFile;

const EXECUTABLE_DIR: &str = "MacOS";
const RESOURCES_DIR: &str = "Resources";
/// Identifiers are reverse-DNS names; `local` marks them as not tied to a real domain.
const BUNDLE_ID_PREFIX: &str = "local.nifty.";

/// Writes `<output>/<name>.app`, replacing an earlier build, and returns its path.
pub(crate) fn write_app(
    output: &Path,
    name: &str,
    files: &[SourceFile],
    runner: &Path,
) -> Result<PathBuf, CliError> {
    let app = output.join(format!("{name}.app"));
    let failed =
        |error: io::Error| CliError::new(format!("can't write {}: {error}", app.display()));
    if app.exists() {
        fs::remove_dir_all(&app).map_err(failed)?;
    }
    let contents = app.join("Contents");
    let executables = contents.join(EXECUTABLE_DIR);
    let resources = contents.join(RESOURCES_DIR);
    fs::create_dir_all(&executables).map_err(failed)?;
    fs::create_dir_all(&resources).map_err(failed)?;
    fs::write(contents.join("Info.plist"), info_plist(name)).map_err(failed)?;
    fs::copy(runner, executables.join(name)).map_err(failed)?;
    write_project(&resources, files).map_err(failed)?;
    Ok(app)
}

/// The project inside the bundle whose executable is `executable`, if it is in one.
pub(crate) fn bundled_project(executable: &Path) -> Option<PathBuf> {
    let executables = executable.parent()?;
    if executables.file_name()? != EXECUTABLE_DIR {
        return None;
    }
    let resources = executables.parent()?.join(RESOURCES_DIR);
    resources.join(ENTRY_FILE).is_file().then_some(resources)
}

fn info_plist(name: &str) -> String {
    let display_name = escape_xml(name);
    let bundle_id = bundle_id(name);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>{display_name}</string>
  <key>CFBundleIdentifier</key>
  <string>{bundle_id}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>{display_name}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
"#
    )
}

/// Bundle identifiers allow only letters, digits, `-` and `.`.
fn bundle_id(name: &str) -> String {
    let suffix: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("{BUNDLE_ID_PREFIX}{suffix}")
}

fn escape_xml(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::sample_project;
    use crate::test_support::scratch_dir;

    #[test]
    fn writes_a_bundle_the_runner_can_find_its_project_in() {
        let dir = scratch_dir("build-macos-bundle");
        let runner = dir.join("nf-runner");
        fs::write(&runner, "runner").expect("write a stand-in runner");

        let app = write_app(
            &dir.join("build"),
            "todos",
            &sample_project("window(\"T\") {}"),
            &runner,
        )
        .expect("the bundle is written");

        assert_eq!(app, dir.join("build/todos.app"));
        let executable = app.join("Contents/MacOS/todos");
        assert_eq!(fs::read_to_string(&executable).expect("runner"), "runner");
        let project = bundled_project(&executable).expect("the project is bundled");
        assert_eq!(
            fs::read_to_string(project.join(ENTRY_FILE)).expect("source"),
            "window(\"T\") {}"
        );
        assert!(project.join("ui/header.nf").is_file());
        let plist = fs::read_to_string(app.join("Contents/Info.plist")).expect("plist");
        assert!(plist.contains("<string>local.nifty.todos</string>"));
    }

    #[test]
    fn building_again_replaces_the_previous_bundle() {
        let dir = scratch_dir("build-macos-rebuild");
        let runner = dir.join("nf-runner");
        fs::write(&runner, "runner").expect("write a stand-in runner");
        let app = write_app(&dir, "app", &sample_project("old"), &runner).expect("first build");
        fs::write(app.join("Contents/Resources/stale.nf"), "").expect("write a stale file");

        write_app(&dir, "app", &sample_project("new"), &runner).expect("second build");

        assert!(!app.join("Contents/Resources/stale.nf").exists());
    }

    #[test]
    fn a_runner_outside_a_bundle_has_no_project() {
        assert_eq!(bundled_project(Path::new("/usr/local/bin/nf-runner")), None);
    }

    #[test]
    fn names_are_escaped_and_identifiers_kept_to_allowed_characters() {
        assert_eq!(bundle_id("my_app 2"), "local.nifty.my-app-2");
        let plist = info_plist("Tom & Jerry");
        assert!(plist.contains("<string>Tom &amp; Jerry</string>"));
    }
}
