use crate::source::{SourceMap, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub message: String,
    pub edits: Vec<(Span, String)>,
    pub applicable: bool,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub notes: Vec<String>,
    pub helps: Vec<String>,
    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(Severity::Error, span, message)
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(Severity::Warning, span, message)
    }

    fn new(severity: Severity, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            severity,
            message: message.into(),
            span,
            notes: Vec::new(),
            helps: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn note(mut self, n: impl Into<String>) -> Diagnostic {
        self.notes.push(n.into());
        self
    }

    pub fn help(mut self, h: impl Into<String>) -> Diagnostic {
        self.helps.push(h.into());
        self
    }

    pub fn fix(mut self, message: impl Into<String>, span: Span, replacement: impl Into<String>) -> Diagnostic {
        self.suggestions.push(Suggestion {
            message: message.into(),
            edits: vec![(span, replacement.into())],
            applicable: true,
        });
        self
    }

    pub fn maybe_fix(mut self, message: impl Into<String>, span: Span, replacement: impl Into<String>) -> Diagnostic {
        self.suggestions.push(Suggestion {
            message: message.into(),
            edits: vec![(span, replacement.into())],
            applicable: false,
        });
        self
    }
}

struct Paint {
    red: &'static str,
    yellow: &'static str,
    blue: &'static str,
    cyan: &'static str,
    green: &'static str,
    bold: &'static str,
    reset: &'static str,
}

fn paint(color: bool) -> Paint {
    if color {
        Paint {
            red: "\x1b[31m",
            yellow: "\x1b[33m",
            blue: "\x1b[34m",
            cyan: "\x1b[36m",
            green: "\x1b[32m",
            bold: "\x1b[1m",
            reset: "\x1b[0m",
        }
    } else {
        Paint {
            red: "",
            yellow: "",
            blue: "",
            cyan: "",
            green: "",
            bold: "",
            reset: "",
        }
    }
}

fn patched_line(sm: &SourceMap, s: &Suggestion) -> Option<(usize, String, String)> {
    let file = s.edits.first()?.0.file;
    if (file as usize) >= sm.files.len() || s.edits.iter().any(|(sp, _)| sp.file != file) {
        return None;
    }
    let f = sm.file(file);
    let (line, _) = f.line_col(s.edits[0].0.start as usize);
    let start = f.line_start(line);
    let text = f.line_text(line);
    let end = start + text.len();
    let mut edits: Vec<&(Span, String)> = s.edits.iter().collect();
    edits.sort_by_key(|(sp, _)| sp.start);
    let mut out = String::new();
    let mut marks = String::new();
    let mut pos = start;
    for (sp, rep) in edits {
        let (a, b) = (sp.start as usize, sp.end as usize);
        if a < pos || b > end || rep.contains('\n') {
            return None;
        }
        let kept = &f.src[pos..a];
        out.push_str(kept);
        marks.push_str(&" ".repeat(kept.chars().count()));
        out.push_str(rep);
        let mark = if a == b { '+' } else { '~' };
        marks.push_str(&mark.to_string().repeat(rep.chars().count().max(if a == b { 0 } else { 1 })));
        pos = b;
    }
    out.push_str(&f.src[pos..end]);
    Some((line, out, marks.trim_end().to_string()))
}

pub fn render(sm: &SourceMap, d: &Diagnostic, color: bool) -> String {
    let p = paint(color);
    let (label, c) = match d.severity {
        Severity::Error => ("error", p.red),
        Severity::Warning => ("warning", p.yellow),
    };
    let mut out = format!("{}{}{}{}: {}{}{}\n", p.bold, c, label, p.reset, p.bold, d.message, p.reset);
    if (d.span.file as usize) >= sm.files.len() {
        for n in &d.notes {
            out.push_str(&format!("  = note: {}\n", n));
        }
        for h in &d.helps {
            out.push_str(&format!("  = help: {}\n", h));
        }
        return out;
    }
    let f = sm.file(d.span.file);
    let (line, col) = f.line_col(d.span.start as usize);
    let (eline, ecol) = f.line_col(d.span.end as usize);
    let mut gutter = line.to_string().len();
    let patches: Vec<Option<(usize, String, String)>> = d.suggestions.iter().map(|s| patched_line(sm, s)).collect();
    for (l, _, _) in patches.iter().flatten() {
        gutter = gutter.max(l.to_string().len());
    }
    let bar = |out: &mut String| out.push_str(&format!("{}{:>w$} |{}\n", p.blue, "", p.reset, w = gutter));
    out.push_str(&format!("{}{:>w$}--> {}{}:{}:{}\n", p.blue, "", p.reset, f.name, line, col, w = gutter));
    if line <= f.line_count() {
        let text = f.line_text(line);
        bar(&mut out);
        out.push_str(&format!("{}{:>w$} |{} {}\n", p.blue, line, p.reset, text, w = gutter));
        let width = if eline == line {
            (ecol.saturating_sub(col)).max(1)
        } else {
            (text.chars().count() + 1).saturating_sub(col).max(1)
        };
        let pad: String = text.chars().take(col - 1).map(|ch| if ch == '\t' { '\t' } else { ' ' }).collect();
        out.push_str(&format!(
            "{}{:>w$} |{} {}{}{}{}\n",
            p.blue,
            "",
            p.reset,
            pad,
            c,
            "^".repeat(width),
            p.reset,
            w = gutter
        ));
    }
    for n in &d.notes {
        out.push_str(&format!("{}{:>w$} = {}{}note{}: {}\n", p.blue, "", p.reset, p.bold, p.reset, n, w = gutter));
    }
    for h in &d.helps {
        out.push_str(&format!("{}{:>w$} = {}{}help{}: {}\n", p.blue, "", p.reset, p.bold, p.reset, h, w = gutter));
    }
    for (s, patch) in d.suggestions.iter().zip(patches) {
        out.push_str(&format!("{}{}help{}: {}\n", p.bold, p.cyan, p.reset, s.message));
        if let Some((l, text, marks)) = patch {
            bar(&mut out);
            out.push_str(&format!("{}{:>w$} |{} {}\n", p.blue, l, p.reset, text, w = gutter));
            if !marks.is_empty() {
                out.push_str(&format!("{}{:>w$} |{} {}{}{}\n", p.blue, "", p.reset, p.green, marks, p.reset, w = gutter));
            }
        }
    }
    out
}

pub fn apply(src: &str, edits: &[(usize, usize, String)]) -> String {
    let mut sorted: Vec<&(usize, usize, String)> = edits.iter().collect();
    sorted.sort_by_key(|e| (e.0, e.1));
    let mut out = String::with_capacity(src.len());
    let mut pos = 0;
    for (a, b, rep) in sorted {
        if *a < pos || *b > src.len() {
            continue;
        }
        out.push_str(&src[pos..*a]);
        out.push_str(rep);
        pos = *b;
    }
    out.push_str(&src[pos..]);
    out
}
