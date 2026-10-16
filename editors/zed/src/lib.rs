use zed_extension_api as zed;

struct OcheExtension;

impl zed::Extension for OcheExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let command = worktree.which("oche").ok_or_else(|| {
            "Cannot find `oche` on the project PATH. Install the Oche CLI or add it to PATH before opening Zed."
                .to_owned()
        })?;
        Ok(zed::Command {
            command,
            args: vec!["lsp".to_owned()],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(OcheExtension);
