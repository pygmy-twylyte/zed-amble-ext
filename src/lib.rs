use zed_extension_api::{self as zed, LanguageServerId, Result};

struct AmbleExtension;

impl zed::Extension for AmbleExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        if language_server_id.as_ref() != "amble-lsp" {
            return Err(format!(
                "Unknown language server: {}",
                language_server_id.as_ref()
            ));
        }

        // The binary is in the extension's bin directory
        // For dev extensions, use the absolute path
        let command = "/home/dave/code/zed-amble-ext/bin/amble-lsp".to_string();

        Ok(zed::Command {
            command,
            args: vec![],
            env: Default::default(),
        })
    }
}

zed::register_extension!(AmbleExtension);
