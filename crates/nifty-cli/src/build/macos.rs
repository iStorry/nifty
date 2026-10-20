//! macOS apps are bundles: a folder ending in `.app` holding the executable and a
//! description of the app.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::CliError;

const EXECUTABLE_DIR: &str = "MacOS";
/// Identifiers are reverse-DNS names; `local` marks them as not tied to a real domain.
const BUNDLE_ID_PREFIX: &str = "local.nifty.";

/// Writes `<output>/<name>.app` around `executable`, replacing an earlier build, and
/// returns its path.
pub(crate) fn write_app(output: &Path, name: &str, executable: &Path) -> Result<PathBuf, CliError> {
    let app = output.join(format!("{name}.app"));
    let failed =
        |error: io::Error| CliError::new(format!("can't write {}: {error}", app.display()));
    if app.exists() {
        fs::remove_dir_all(&app).map_err(failed)?;
    }
    let contents = app.join("Contents");
    let executables = contents.join(EXECUTABLE_DIR);
    fs::create_dir_all(&executables).map_err(failed)?;
    fs::write(contents.join("Info.plist"), info_plist(name)).map_err(failed)?;
    fs::copy(executable, executables.join(name)).map_err(failed)?;
    Ok(app)
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
    use crate::test_support::scratch_dir;

    #[test]
    fn writes_a_bundle_around_the_executable() {
        let dir = scratch_dir("build-macos-bundle");
        let executable = dir.join("app");
        fs::write(&executable, "machine code").expect("write a stand-in executable");

        let app =
            write_app(&dir.join("build"), "todos", &executable).expect("the bundle is written");

        assert_eq!(app, dir.join("build/todos.app"));
        let bundled = app.join("Contents/MacOS/todos");
        assert_eq!(
            fs::read_to_string(bundled).expect("executable"),
            "machine code"
        );
        let plist = fs::read_to_string(app.join("Contents/Info.plist")).expect("plist");
        assert!(plist.contains("<string>local.nifty.todos</string>"));
    }

    #[test]
    fn building_again_replaces_the_previous_bundle() {
        let dir = scratch_dir("build-macos-rebuild");
        let executable = dir.join("app");
        fs::write(&executable, "machine code").expect("write a stand-in executable");
        let app = write_app(&dir, "app", &executable).expect("first build");
        fs::write(app.join("Contents/stale"), "").expect("write a stale file");

        write_app(&dir, "app", &executable).expect("second build");

        assert!(!app.join("Contents/stale").exists());
    }

    #[test]
    fn names_are_escaped_and_identifiers_kept_to_allowed_characters() {
        assert_eq!(bundle_id("my_app 2"), "local.nifty.my-app-2");
        let plist = info_plist("Tom & Jerry");
        assert!(plist.contains("<string>Tom &amp; Jerry</string>"));
    }
}
