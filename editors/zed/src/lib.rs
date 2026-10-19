//! Starts `nf lsp` for `.nf` files. The binary comes from `lsp.nifty.binary` in the Zed
//! settings, or else from `PATH`.

use zed_extension_api::settings::LspSettings;
use zed_extension_api::{self as zed, Command, LanguageServerId, Result, Worktree};

const SERVER_BINARY: &str = "nf";
const SERVER_ARGUMENTS: &[&str] = &["lsp"];

struct NiftyExtension;

impl zed::Extension for NiftyExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Command> {
        let binary = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?.binary;
        let configured_path = binary.as_ref().and_then(|binary| binary.path.clone());
        let command = configured_path
            .or_else(|| worktree.which(SERVER_BINARY))
            .ok_or_else(|| {
                format!(
                    "`{SERVER_BINARY}` was not found on PATH; install it or set \
                     `lsp.{language_server_id}.binary.path` in your Zed settings"
                )
            })?;
        let args = binary
            .and_then(|binary| binary.arguments)
            .unwrap_or_else(|| SERVER_ARGUMENTS.iter().map(ToString::to_string).collect());
        Ok(Command {
            command,
            args,
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(NiftyExtension);
