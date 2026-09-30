use std::path::Path;
use std::process::ExitCode;

#[derive(Clone, Debug, PartialEq)]
enum T {
    Word(String),
    Num(String),
    Str(String),
    Comment(String),
    Op(String),
    Open(char, bool),
    Close(char, bool),
    Comma,
    Semi,
    Colon,
    Dot,
}

const KEYWORDS_SPACE: &[&str] = &[
    "if", "while", "for", "return", "else", "import", "fun", "var", "const", "def", "pub", "priv", "async", "await", "in", "is", "as", "static",
];

fn lex_line(line: &str) -> Vec<T> {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' || c == '\t' {
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            out.push(T::Comment(chars[i..].iter().collect::<String>().trim_end().to_string()));
            break;
        }
        if c == '"' || c == '\'' {
            let start = i;
            i += 1;
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
                    depth += 1;
                    i += 2;
                    continue;
                }
                if depth > 0 && chars[i] == '}' {
                    depth -= 1;
                    i += 1;
                    continue;
                }
                if depth == 0 && chars[i] == c {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(T::Str(chars[start..i.min(chars.len())].iter().collect()));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(T::Word(chars[start..i].iter().collect()));
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric()
                    || chars[i] == '_'
                    || (chars[i] == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
                    || ((chars[i] == '-' || chars[i] == '+') && matches!(chars[i - 1], 'e' | 'E') && !chars[start..i].iter().any(|c| *c == 'x' || *c == 'X')))
            {
                i += 1;
            }
            out.push(T::Num(chars[start..i].iter().collect()));
            continue;
        }
        let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
        let three: String = chars[i..(i + 3).min(chars.len())].iter().collect();
        if three == "..=" {
            out.push(T::Op(three));
            i += 3;
            continue;
        }
        if ["==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=", "%=", "..", "->", "!!", "::"].contains(&two.as_str()) {
            out.push(T::Op(two));
            i += 2;
            continue;
        }
        match c {
            '(' | '[' | '{' => out.push(T::Open(c, i + 1 < chars.len() && !chars[i + 1].is_whitespace())),
            ')' | ']' | '}' => out.push(T::Close(c, i > 0 && !chars[i - 1].is_whitespace())),
            ',' => out.push(T::Comma),
            ';' => out.push(T::Semi),
            ':' => out.push(T::Colon),
            '.' => out.push(T::Dot),
            _ => out.push(T::Op(c.to_string())),
        }
        i += 1;
    }
    out
}

fn is_operand_end(t: &T) -> bool {
    match t {
        T::Word(w) => !KEYWORDS_SPACE.contains(&w.as_str()) || w == "self",
        T::Num(_) | T::Str(_) => true,
        T::Close(..) => true,
        T::Op(o) => o == "?" || o == "!!",
        _ => false,
    }
}

fn is_struct_head(toks: &[T]) -> bool {
    let words: Vec<&str> = toks
        .iter()
        .take(5)
        .map(|t| match t {
            T::Word(w) => w.as_str(),
            _ => "",
        })
        .collect();
    let start = if matches!(words.first(), Some(&"pub") | Some(&"priv")) { 1 } else { 0 };
    words.get(start) == Some(&"def") && words.iter().skip(start + 1).take(2).any(|w| *w == "struct")
}

fn render_line(toks: &[T]) -> String {
    let mut out = String::new();
    let mut prev: Option<&T> = None;
    let mut generic_depth = 0;
    let head = is_struct_head(toks);
    let mut parens = 0;
    for (idx, t) in toks.iter().enumerate() {
        let space = match (prev, t) {
            (None, _) => false,
            (_, T::Colon) if head && parens == 0 => true,
            (_, T::Comment(_)) => true,
            (Some(T::Op(o)), _) if o == "@" => false,
            (Some(T::Open('{', tight)), _) => !tight && !matches!(t, T::Close('}', _)),
            (Some(T::Open(..)), _) => false,
            (_, T::Close('}', tight)) => !tight && !matches!(prev, Some(T::Open('{', _))),
            (_, T::Close(..)) => false,
            (_, T::Comma) | (_, T::Semi) | (_, T::Colon) => false,
            (Some(T::Comma), _) | (Some(T::Semi), _) | (Some(T::Colon), _) => true,
            (_, T::Dot) | (Some(T::Dot), _) => false,
            (Some(T::Op(o)), _) if o == ".." || o == "..=" => false,
            (_, T::Op(o)) if o == ".." || o == "..=" => false,
            (Some(T::Op(o)), _) if o == "!!" => !matches!(t, T::Dot | T::Open('(', _) | T::Open('[', _)),
            (_, T::Op(o)) if o == "!!" || o == "?" => false,
            (Some(T::Op(o)), T::Open('(', _)) if o == "!" => false,
            (Some(T::Word(w)), T::Open('(', _)) => KEYWORDS_SPACE.contains(&w.as_str()) && w != "fun",
            (Some(T::Word(w)), T::Op(o)) if o == "<" && (w == "Future" || w == "Task" || w == "Map" || w == "Array") => {
                generic_depth += 1;
                false
            }
            (Some(T::Op(o)), _) if o == "<" && generic_depth > 0 => false,
            (_, T::Op(o)) if o == ">" && generic_depth > 0 => {
                generic_depth -= 1;
                false
            }
            (Some(_), T::Open('{', _)) => true,
            (Some(_), T::Open('(', _)) | (Some(_), T::Open('[', _)) => match prev {
                Some(p) => !is_operand_end(p) || matches!(p, T::Word(w) if KEYWORDS_SPACE.contains(&w.as_str())),
                None => false,
            },
            (Some(T::Op(o)), _) if (o == "-" || o == "!") => {
                let before = if idx >= 2 { Some(&toks[idx - 2]) } else { None };
                matches!(before, Some(b) if is_operand_end(b)) && o == "-"
            }
            (Some(T::Close(..)), T::Word(_)) => true,
            _ => true,
        };
        if space && !out.is_empty() {
            out.push(' ');
        }
        match t {
            T::Word(s) | T::Num(s) | T::Str(s) | T::Comment(s) | T::Op(s) => out.push_str(s),
            T::Open(c, _) | T::Close(c, _) => out.push(*c),
            T::Comma => out.push(','),
            T::Semi => out.push(';'),
            T::Colon => out.push(':'),
            T::Dot => out.push('.'),
        }
        match t {
            T::Open('(', _) => parens += 1,
            T::Close(')', _) => parens -= 1,
            _ => {}
        }
        prev = Some(t);
    }
    out
}

fn split_segments(toks: Vec<T>) -> Vec<Vec<T>> {
    let n = toks.len();
    let mut open_matched = vec![false; n];
    let mut close_local = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        match t {
            T::Open('{', _) => stack.push(i),
            T::Close('}', _) => {
                if let Some(o) = stack.pop() {
                    open_matched[o] = true;
                    close_local[i] = true;
                }
            }
            _ => {}
        }
    }
    let mut segs = Vec::new();
    let mut cur: Vec<T> = Vec::new();
    for (i, t) in toks.into_iter().enumerate() {
        let is_close = matches!(t, T::Close('}', _));
        let is_open = matches!(t, T::Open('{', _));
        if is_close && !close_local[i] && !cur.is_empty() {
            segs.push(std::mem::take(&mut cur));
        }
        cur.push(t);
        if is_open && !open_matched[i] && i + 1 < n {
            segs.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        segs.push(cur);
    }
    let mut merged: Vec<Vec<T>> = Vec::new();
    for seg in segs {
        if seg.len() == 1 && matches!(seg[0], T::Comment(_)) {
            if let Some(last) = merged.last_mut() {
                last.extend(seg);
                continue;
            }
        }
        merged.push(seg);
    }
    merged
}

pub fn format(src: &str) -> String {
    let mut out = String::new();
    let mut depth: i32 = 0;
    let mut blank = 0;
    let mut in_block_comment = false;
    let mut started = false;
    let mut last_was_close = false;
    for raw in src.lines() {
        let line = raw.trim();
        if in_block_comment {
            if line.starts_with('*') {
                out.push_str(&"    ".repeat(depth.max(0) as usize));
                out.push(' ');
                out.push_str(line);
            } else {
                out.push_str(raw.trim_end());
            }
            out.push('\n');
            if line.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if line.starts_with("/*") {
            if (started && blank > 0) || last_was_close {
                out.push('\n');
            }
            blank = 0;
            last_was_close = false;
            started = true;
            out.push_str(&"    ".repeat(depth.max(0) as usize));
            out.push_str(line);
            out.push('\n');
            in_block_comment = !line.contains("*/");
            continue;
        }
        if line.is_empty() {
            blank += 1;
            continue;
        }
        let toks = lex_line(line);
        let first = started && blank > 0;
        blank = 0;
        for (si, seg) in split_segments(toks).into_iter().enumerate() {
            let leading_close = seg.iter().take_while(|t| matches!(t, T::Close(..))).count() as i32;
            let mut delta = 0;
            for t in &seg {
                match t {
                    T::Open(..) => delta += 1,
                    T::Close(..) => delta -= 1,
                    _ => {}
                }
            }
            let is_decl = matches!(seg.first(), Some(T::Word(w)) if matches!(w.as_str(), "fun" | "static" | "def" | "async" | "pub" | "priv"))
                || matches!(seg.first(), Some(T::Op(o)) if o == "@");
            if (si == 0 && first && !(leading_close > 0 && seg.len() == 1)) || (is_decl && last_was_close) {
                out.push('\n');
            }
            last_was_close = seg.len() == 1 && matches!(seg[0], T::Close('}', _));
            let indent = (depth - leading_close).max(0) as usize;
            out.push_str(&"    ".repeat(indent));
            out.push_str(&render_line(&seg));
            out.push('\n');
            depth += delta;
        }
        started = true;
    }
    out
}

pub fn cmd(args: &[String]) -> ExitCode {
    let mut write = false;
    let mut check = false;
    let mut files = Vec::new();
    for a in args {
        match a.as_str() {
            "-w" | "--write" => write = true,
            "--check" => check = true,
            _ => files.push(a.clone()),
        }
    }
    if files.is_empty() {
        let mut src = String::new();
        if std::io::Read::read_to_string(&mut std::io::stdin(), &mut src).is_err() {
            return ExitCode::from(1);
        }
        print!("{}", format(&src));
        return ExitCode::SUCCESS;
    }
    let mut code = ExitCode::SUCCESS;
    for f in files {
        let path = Path::new(&f);
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {}: {}", f, e);
                code = ExitCode::from(1);
                continue;
            }
        };
        let formatted = format(&src);
        if check {
            if formatted != src {
                println!("{} is not formatted", f);
                code = ExitCode::from(1);
            }
        } else if write {
            if formatted != src {
                if let Err(e) = std::fs::write(path, &formatted) {
                    eprintln!("error: cannot write {}: {}", f, e);
                    code = ExitCode::from(1);
                }
            }
        } else {
            print!("{}", formatted);
        }
    }
    code
}
