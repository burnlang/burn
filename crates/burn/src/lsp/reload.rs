use super::json::Json;
use super::{notify, uri_to_path, Server};
use std::path::{Path, PathBuf};

pub const RELOAD: &str = "burn.server.reloadProject";

fn ash_path() -> PathBuf {
    let exe = if cfg!(windows) { "ash.exe" } else { "ash" };
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        if dir.join(exe).is_file() {
            return dir.join(exe);
        }
    }
    let home = crate::project::burn_home().join("bin").join(exe);
    if home.is_file() {
        return home;
    }
    PathBuf::from(exe)
}

fn show(kind: i32, message: String) {
    notify(
        "window/showMessage",
        Json::obj(vec![("type", Json::num(kind as f64)), ("message", Json::str(message))]),
    );
}

fn needs_reload(message: &str) -> bool {
    message.contains("ash install") || message.contains("not downloaded") || message.contains("not installed yet")
}

impl Server {
    fn project_dir(&self, hint: Option<&str>) -> Option<PathBuf> {
        if let Some(root) = hint.map(uri_to_path).and_then(|p| crate::project::find_root(&p)) {
            return Some(root);
        }
        self.roots
            .iter()
            .chain(self.docs.keys().map(|u| uri_to_path(u)).collect::<Vec<_>>().iter())
            .find_map(|p| crate::project::find_root(p))
    }

    pub fn reload_project(&mut self, params: &Json) -> Json {
        let hint = params.get("arguments").as_arr().first().and_then(|a| a.as_str()).map(|s| s.to_string());
        let Some(dir) = self.project_dir(hint.as_deref()) else {
            show(1, "Burn: no burn.toml found, so there is no project to reload".into());
            return Json::Null;
        };
        let ash = ash_path();
        let result = std::process::Command::new(&ash)
            .arg("sync")
            .current_dir(&dir)
            .env("NO_COLOR", "1")
            .stdin(std::process::Stdio::null())
            .output();
        match result {
            Ok(out) if out.status.success() => show(3, format!("Burn: reloaded {}", project_name(&dir))),
            Ok(out) => {
                let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                let tail: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).rev().take(4).collect();
                let tail: Vec<&str> = tail.into_iter().rev().collect();
                show(1, format!("Burn: `ash sync` failed in {}\n{}", dir.display(), tail.join("\n")));
            }
            Err(e) => show(
                1,
                format!(
                    "Burn: cannot run {} ({}). Install ash with burnup, or run `ash sync` in {}",
                    ash.display(),
                    e,
                    dir.display()
                ),
            ),
        }
        self.reanalyze_all();
        Json::Null
    }

    pub fn reanalyze_all(&mut self) {
        let open: Vec<String> = self.docs.keys().cloned().collect();
        for u in open {
            self.analyze(&u);
        }
    }

    pub fn reload_actions(&self, uri: &str, params: &Json) -> Vec<Json> {
        let mut out = Vec::new();
        for d in params.at(&["context", "diagnostics"]).as_arr() {
            if !needs_reload(d.get("message").as_str().unwrap_or("")) {
                continue;
            }
            let title = "Reload project (ash sync)";
            out.push(Json::obj(vec![
                ("title", Json::str(title)),
                ("kind", Json::str("quickfix")),
                ("isPreferred", Json::Bool(true)),
                ("diagnostics", Json::Arr(vec![d.clone()])),
                (
                    "command",
                    Json::obj(vec![
                        ("title", Json::str(title)),
                        ("command", Json::str(RELOAD)),
                        ("arguments", Json::Arr(vec![Json::str(uri)])),
                    ]),
                ),
            ]));
            break;
        }
        out
    }
}

fn project_name(dir: &Path) -> String {
    crate::project::load(dir).map(|p| p.manifest.name).unwrap_or_else(|_| dir.display().to_string())
}

pub fn wants_watchers(init: &Json) -> bool {
    init.at(&["capabilities", "workspace", "didChangeWatchedFiles", "dynamicRegistration"])
        .as_bool()
        == Some(true)
}

pub fn register_watchers() {
    let watcher = |glob: &str| Json::obj(vec![("globPattern", Json::str(glob))]);
    super::send(&Json::obj(vec![
        ("jsonrpc", Json::str("2.0")),
        ("id", Json::str("burn-watch")),
        ("method", Json::str("client/registerCapability")),
        (
            "params",
            Json::obj(vec![(
                "registrations",
                Json::Arr(vec![Json::obj(vec![
                    ("id", Json::str("burn-watch")),
                    ("method", Json::str("workspace/didChangeWatchedFiles")),
                    (
                        "registerOptions",
                        Json::obj(vec![(
                            "watchers",
                            Json::Arr(vec![watcher("**/burn.toml"), watcher("**/burn.lock"), watcher("**/*.bn")]),
                        )]),
                    ),
                ])]),
            )]),
        ),
    ]));
}
