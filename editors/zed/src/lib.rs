use zed_extension_api::{self as zed, settings::LspSettings, LanguageServerId, Result};

struct BurnExtension;

impl BurnExtension {
    fn find_burn(worktree: &zed::Worktree) -> String {
        if let Some(path) = worktree.which("burn") {
            return path;
        }
        let env = worktree.shell_env();
        let var = |name: &str| env.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
        if let Some(home) = var("BURN_HOME") {
            return format!("{}/bin/burn", home);
        }
        match var("HOME") {
            Some(home) => format!("{}/.burn/bin/burn", home),
            None => "burn".to_string(),
        }
    }
}

impl zed::Extension for BurnExtension {
    fn new() -> Self {
        BurnExtension
    }

    fn language_server_command(&mut self, _id: &LanguageServerId, worktree: &zed::Worktree) -> Result<zed::Command> {
        let binary = LspSettings::for_worktree("burn", worktree).ok().and_then(|s| s.binary);
        let command = binary.as_ref().and_then(|b| b.path.clone()).unwrap_or_else(|| Self::find_burn(worktree));
        let args = binary.and_then(|b| b.arguments).unwrap_or_else(|| vec!["lsp".to_string()]);
        Ok(zed::Command {
            command,
            args,
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(BurnExtension);
