use std::path::Path;
use std::process::ExitCode;

#[derive(Clone, Debug, PartialEq)]
enum T {
    Word(String),
    Num(String),
    Str(String),
    Comment(String),
    Op(String),
    Open(char),
    Close(char),
    Comma,
    Semi,
    Colon,
    Dot,
}

const KEYWORDS_SPACE: &[&str] = &["if", "while", "for", "return", "else", "import", "fun", "var", "const", "def", "pub", "priv", "async", "await", "in", "is", "as", "static"];

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
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || (chars[i] == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())) {
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
        if ["==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=", "%=", "..", "->", "!!"].contains(&two.as_str()) {
            out.push(T::Op(two));
            i += 2;
            continue;
        }
        match c {
            '(' | '[' | '{' => out.push(T::Open(c)),
            ')' | ']' | '}' => out.push(T::Close(c)),
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
        T::Close(_) => true,
        T::Op(o) => o == "?" || o == "!!",
        _ => false,
    }
}

fn render_line(toks: &[T]) -> String {
    let mut out = String::new();
    let mut prev: Option<&T> = None;
    let mut generic_depth = 0;
    for (idx, t) in toks.iter().enumerate() {
        let space = match (prev, t) {
            (None, _) => false,
            (_, T::Comment(_)) => true,
            (Some(T::Open('{')), _) => !matches!(t, T::Close('}')),
            (Some(T::Open(_)), _) => false,
            (_, T::Close('}')) => !matches!(prev, Some(T::Open('{'))),
            (_, T::Close(_)) => false,
            (_, T::Comma) | (_, T::Semi) | (_, T::Colon) => false,
            (Some(T::Comma), _) | (Some(T::Semi), _) | (Some(T::Colon), _) => true,
            (_, T::Dot) | (Some(T::Dot), _) => false,
            (Some(T::Op(o)), _) if o == ".." || o == "..=" => false,
            (_, T::Op(o)) if o == ".." || o == "..=" => false,
            (Some(T::Op(o)), _) if o == "!!" => !matches!(t, T::Dot | T::Open('(') | T::Open('[')),
            (_, T::Op(o)) if o == "!!" || o == "?" => false,
            (Some(T::Op(o)), T::Open('(')) if o == "!" => false,
            (Some(T::Word(w)), T::Open('(')) => KEYWORDS_SPACE.contains(&w.as_str()) && w != "fun",
            (Some(T::Word(w)), T::Op(o)) if o == "<" && (w == "Future" || w == "Task" || w == "Map" || w == "Array") => {
                generic_depth += 1;
                false
            }
            (Some(T::Op(o)), _) if o == "<" && generic_depth > 0 => false,
            (_, T::Op(o)) if o == ">" && generic_depth > 0 => {
                generic_depth -= 1;
                false
            }
            (Some(_), T::Open('{')) => true,
            (Some(_), T::Open('(')) | (Some(_), T::Open('[')) => match prev {
                Some(p) => !is_operand_end(p) || matches!(p, T::Word(w) if KEYWORDS_SPACE.contains(&w.as_str())),
                None => false,
            },
            (Some(T::Op(o)), _) if (o == "-" || o == "!") => {
                let before = if idx >= 2 { Some(&toks[idx - 2]) } else { None };
                matches!(before, Some(b) if is_operand_end(b)) && o == "-"
            }
            (Some(T::Close(_)), T::Word(_)) => true,
            _ => true,
        };
        if space && !out.is_empty() {
            out.push(' ');
        }
        match t {
            T::Word(s) | T::Num(s) | T::Str(s) | T::Comment(s) | T::Op(s) => out.push_str(s),
            T::Open(c) | T::Close(c) => out.push(*c),
            T::Comma => out.push(','),
            T::Semi => out.push(';'),
            T::Colon => out.push(':'),
            T::Dot => out.push('.'),
        }
        prev = Some(t);
    }
    out
}

pub fn format(src: &str) -> String {
    let mut out = String::new();
    let mut depth: i32 = 0;
    let mut blank = 0;
    let mut in_block_comment = false;
    let mut started = false;
    for raw in src.lines() {
        let line = raw.trim();
        if in_block_comment {
            out.push_str(raw.trim_end());
            out.push('\n');
            if line.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if line.starts_with("/*") {
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
        let leading_close = toks.iter().take_while(|t| matches!(t, T::Close(_))).count() as i32;
        let mut delta = 0;
        for t in &toks {
            match t {
                T::Open(_) => delta += 1,
                T::Close(_) => delta -= 1,
                _ => {}
            }
        }
        if started && blank > 0 && !(leading_close > 0 && toks.len() == 1) {
            out.push('\n');
        }
        blank = 0;
        let indent = (depth - leading_close).max(0) as usize;
        out.push_str(&"    ".repeat(indent));
        out.push_str(&render_line(&toks));
        out.push('\n');
        depth += delta;
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
