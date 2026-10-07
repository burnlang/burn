use super::Checker;
use crate::diag::Diagnostic;
use crate::loader::Loaded;
use crate::source::Span;
use std::collections::HashSet;

pub fn std_only(builtin: &str) -> Option<&'static str> {
    Some(match builtin {
        "parseJSON" | "toJSON" => "working with JSON",
        "__httpRequest" => "making HTTP requests",
        "__exec" => "running processes",
        "__fsOp" | "__listDir" => "changing files and directories",
        "__cwd" => "reading the working directory",
        "__localTime" => "reading the local time zone",
        _ => return None,
    })
}

pub const HELP: &str = "this program is built without the standard library (`std = false` in burn.toml, or `--no-std`); only the built-in core is available, so remove that setting to use it";

impl<'a> Checker<'a> {
    pub fn needs_std(&mut self, span: Span, what: &'static str) {
        if self.no_std && !self.is_dry() {
            self.std_uses.push((span, what));
        }
    }

    pub fn check_no_std(&mut self, loaded: &Loaded) {
        if !self.no_std {
            return;
        }
        let async_fns: Vec<Span> = self.funcs.iter().filter(|f| f.is_async).map(|f| f.span).collect();
        for s in async_fns {
            self.std_uses.push((s, "an `async fun`"));
        }
        let is_std = |i: usize| loaded.modules[i].key.starts_with("std:");
        let module_of = |file| loaded.modules.iter().position(|m| m.file == file);
        let uses = std::mem::take(&mut self.std_uses);
        let mut seen = HashSet::new();
        for (span, what) in uses {
            if module_of(span.file).map(is_std).unwrap_or(false) {
                continue;
            }
            if seen.insert((span, what)) {
                self.emit(Diagnostic::error(span, format!("{} needs the standard library", what)).help(HELP));
            }
        }
        for (i, m) in loaded.modules.iter().enumerate() {
            if is_std(i) {
                continue;
            }
            for (d, span) in &m.imports {
                if is_std(*d) {
                    let name = loaded.modules[*d].key.trim_start_matches("std:");
                    self.emit(Diagnostic::error(*span, format!("`std/{}` is part of the standard library", name)).help(HELP));
                }
            }
            for (_, span) in &m.libs {
                self.emit(Diagnostic::error(*span, "bytecode libraries need bvm, which is part of the standard library").help(HELP));
            }
        }
    }
}
