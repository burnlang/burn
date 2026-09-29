use crate::source::{SourceMap, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic { severity: Severity::Error, message: message.into(), span, notes: Vec::new() }
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic { severity: Severity::Warning, message: message.into(), span, notes: Vec::new() }
    }

    pub fn note(mut self, n: impl Into<String>) -> Diagnostic {
        self.notes.push(n.into());
        self
    }
}

pub fn render(sm: &SourceMap, d: &Diagnostic, color: bool) -> String {
    let (red, yellow, blue, bold, reset) = if color {
        ("\x1b[31m", "\x1b[33m", "\x1b[34m", "\x1b[1m", "\x1b[0m")
    } else {
        ("", "", "", "", "")
    };
    let (label, c) = match d.severity {
        Severity::Error => ("error", red),
        Severity::Warning => ("warning", yellow),
    };
    let mut out = format!("{}{}{}{}: {}{}\n", bold, c, label, reset, bold, d.message);
    out.push_str(reset);
    if (d.span.file as usize) < sm.files.len() {
        let f = sm.file(d.span.file);
        let (line, col) = f.line_col(d.span.start as usize);
        let (eline, ecol) = f.line_col(d.span.end as usize);
        let gutter = line.to_string().len();
        out.push_str(&format!("{}{:>w$}--> {}{}:{}:{}\n", blue, "", reset, f.name, line, col, w = gutter));
        if line <= f.line_count() {
            let text = f.line_text(line);
            out.push_str(&format!("{}{:>w$} |{}\n", blue, "", reset, w = gutter));
            out.push_str(&format!("{}{} |{} {}\n", blue, line, reset, text));
            let width = if eline == line { (ecol.saturating_sub(col)).max(1) } else { (text.chars().count() + 1).saturating_sub(col).max(1) };
            let pad: String = text.chars().take(col - 1).map(|ch| if ch == '\t' { '\t' } else { ' ' }).collect();
            out.push_str(&format!("{}{:>w$} |{} {}{}{}{}\n", blue, "", reset, pad, c, "^".repeat(width), reset, w = gutter));
        }
        for n in &d.notes {
            out.push_str(&format!("{}{:>w$} = {}note: {}\n", blue, "", reset, n, w = gutter));
        }
    } else {
        for n in &d.notes {
            out.push_str(&format!("  = note: {}\n", n));
        }
    }
    out
}
