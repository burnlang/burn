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

const HELP: &str = "this program is built without the standard runtime (`std = false` in burn.toml, or `--no-std`); remove that setting to use it";

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
        let mut needs: Vec<Option<&'static str>> = vec![None; loaded.modules.len()];
        let uses = std::mem::take(&mut self.std_uses);
        let mut seen = HashSet::new();
        for (span, what) in uses {
            match module_of(span.file) {
                Some(m) if is_std(m) => {
                    needs[m].get_or_insert(what);
                }
                _ => {
                    if seen.insert((span, what)) {
                        self.emit(Diagnostic::error(span, format!("{} needs the standard runtime", what)).help(HELP));
                    }
                }
            }
        }
        let mut changed = true;
        while changed {
            changed = false;
            for i in 0..loaded.modules.len() {
                if !is_std(i) || needs[i].is_some() {
                    continue;
                }
                if let Some(w) = loaded.modules[i].imports.iter().find_map(|(d, _)| needs[*d]) {
                    needs[i] = Some(w);
                    changed = true;
                }
            }
        }
        for (i, m) in loaded.modules.iter().enumerate() {
            if is_std(i) {
                continue;
            }
            for (d, span) in &m.imports {
                let name = loaded.modules[*d].key.trim_start_matches("std:");
                let has_builtins = super::builtins::HOMES.iter().any(|(home, _)| *home == name);
                if has_builtins && !self.import_uses.borrow().contains(&(i, *d)) {
                    continue;
                }
                if let Some(what) = needs[*d] {
                    let name = loaded.modules[*d].key.trim_start_matches("std:").to_string();
                    self.emit(Diagnostic::error(*span, format!("`std/{}` needs the standard runtime for {}", name, what)).help(HELP));
                }
            }
            for (_, span) in &m.libs {
                self.emit(Diagnostic::error(*span, "bytecode libraries need bvm, which is part of the standard runtime").help(HELP));
            }
        }
    }
}
