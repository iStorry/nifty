//! Links a compiled app's object file with the Nifty runtime library into an executable,
//! with the system's own linker.

use std::path::Path;
use std::process::Command;

use crate::CodegenError;

/// The file name of the runtime library every compiled app links against, as `nifty-rt`
/// builds it; `nf` is installed next to it.
#[cfg(target_os = "macos")]
pub const RUNTIME_LIBRARY: &str = "libnifty_rt.a";
#[cfg(windows)]
pub const RUNTIME_LIBRARY: &str = "nifty_rt.lib";

/// Links `object` with `runtime`, the runtime library, into the executable `executable`,
/// replacing it.
///
/// # Errors
/// When the linker can't be found or run, or it fails.
pub fn link(object: &Path, runtime: &Path, executable: &Path) -> Result<(), CodegenError> {
    #[cfg(target_os = "macos")]
    let mut linker = linker(object, runtime, executable);
    #[cfg(windows)]
    let mut linker = linker(object, runtime, executable)?;
    let output = linker.output().map_err(|error| {
        CodegenError::new(format!("can't run the linker: {error}{LINKER_HINT}"))
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(CodegenError::new(format!(
            "linking {} failed:\n{}",
            executable.display(),
            String::from_utf8_lossy(&output.stderr).trim_end()
        )))
    }
}

/// What to do when the linker can't be run at all.
#[cfg(target_os = "macos")]
const LINKER_HINT: &str = "; install Xcode's command line tools with `xcode-select --install`";
#[cfg(windows)]
const LINKER_HINT: &str = "";

/// Xcode's command line tools provide `cc`, which drives the system linker.
#[cfg(target_os = "macos")]
fn linker(object: &Path, runtime: &Path, executable: &Path) -> Command {
    let mut command = Command::new("cc");
    command
        // Leaves out the parts of the runtime library the app never reaches, and the names
        // of its private functions, which only a debugger reads.
        .args(["-Wl,-dead_strip", "-Wl,-x"])
        .arg("-o")
        .arg(executable)
        .arg(object)
        .arg(runtime)
        .args(nifty_rt::NATIVE_LIBS);
    command
}

/// Installs the Build Tools for Visual Studio with the C++ workload, which has the linker.
#[cfg(windows)]
const INSTALL_BUILD_TOOLS: &str = "winget install Microsoft.VisualStudio.2022.BuildTools \
     --override \"--passive --wait --add Microsoft.VisualStudio.Workload.VCTools \
     --includeRecommended\"";

/// The MSVC linker, found where Visual Studio or its Build Tools installed it.
#[cfg(windows)]
fn linker(object: &Path, runtime: &Path, executable: &Path) -> Result<Command, CodegenError> {
    /// Selects Common Controls 6 and per-monitor DPI awareness, as for `nf` itself.
    const MANIFEST: &str = include_str!("../../nifty-cli/windows/nifty.manifest");
    #[cfg(target_arch = "x86_64")]
    const TARGET: &str = "x86_64-pc-windows-msvc";
    #[cfg(target_arch = "aarch64")]
    const TARGET: &str = "aarch64-pc-windows-msvc";

    let tool = cc::windows_registry::find_tool(TARGET, "link.exe").ok_or_else(|| {
        CodegenError::new(format!(
            "building apps needs Microsoft's C++ Build Tools, which link them; install them \
             with `{INSTALL_BUILD_TOOLS}`"
        ))
    })?;
    let manifest = object.with_file_name("nifty.manifest");
    std::fs::write(&manifest, MANIFEST).map_err(|error| {
        CodegenError::new(format!("can't write {}: {error}", manifest.display()))
    })?;
    let mut command = tool.to_command();
    command
        .arg("/NOLOGO")
        .arg(format!("/OUT:{}", executable.display()))
        .arg(object)
        .arg(runtime)
        .args(nifty_rt::NATIVE_LIBS)
        .args([
            "/SUBSYSTEM:WINDOWS",
            "/ENTRY:mainCRTStartup",
            "/MANIFEST:EMBED",
        ])
        .arg(format!("/MANIFESTINPUT:{}", manifest.display()));
    Ok(command)
}
