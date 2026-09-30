#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Para(String),
    Code(String),
    List(Vec<String>),
}

#[derive(Clone, Debug, Default)]
pub struct DocComment {
    pub summary: String,
    pub body: Vec<Block>,
    pub params: Vec<(String, String)>,
    pub ret: Option<String>,
    pub throws: Vec<String>,
    pub see: Vec<String>,
    pub since: Option<String>,
    pub deprecated: Option<String>,
    pub examples: Vec<String>,
    pub authors: Vec<String>,
    pub group: Option<String>,
    pub aliases: Vec<String>,
}

impl DocComment {
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.iter().find(|(n, _)| n == name).map(|(_, d)| d.as_str())
    }
}

fn strip_line(l: &str) -> String {
    let t = l.trim_start();
    let t = t.strip_prefix('*').unwrap_or(t);
    t.strip_prefix(' ').unwrap_or(t).trim_end().to_string()
}

pub fn raw_lines(raw: &str) -> Vec<String> {
    let inner = raw.trim();
    let inner = inner.strip_prefix("/**").unwrap_or(inner);
    let inner = inner.strip_suffix("*/").unwrap_or(inner);
    let mut lines: Vec<String> = inner.lines().map(strip_line).collect();
    while lines.first().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.remove(0);
    }
    while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines
}

fn dedent(lines: &[String]) -> String {
    let n = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut out: Vec<&str> = lines.iter().map(|l| if l.len() >= n { &l[n..] } else { l.trim_start() }).collect();
    while out.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
}

fn blocks(lines: &[String]) -> Vec<Block> {
    let mut out = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut i = 0;
    let flush = |para: &mut Vec<String>, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::Para(para.join(" ")));
            para.clear();
        }
    };
    while i < lines.len() {
        let l = &lines[i];
        if l.trim().is_empty() {
            flush(&mut para, &mut out);
            i += 1;
        } else if l.trim_start().starts_with("```") {
            flush(&mut para, &mut out);
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i].clone());
                i += 1;
            }
            i += 1;
            out.push(Block::Code(dedent(&code)));
        } else if l.starts_with("    ") && para.is_empty() {
            let mut code = Vec::new();
            while i < lines.len() && (lines[i].starts_with("    ") || lines[i].trim().is_empty()) {
                code.push(lines[i].clone());
                i += 1;
            }
            out.push(Block::Code(dedent(&code)));
        } else if l.trim_start().starts_with("- ") {
            flush(&mut para, &mut out);
            let mut items: Vec<String> = Vec::new();
            while i < lines.len() && !lines[i].trim().is_empty() {
                let t = lines[i].trim_start();
                match t.strip_prefix("- ") {
                    Some(rest) => items.push(rest.to_string()),
                    None => {
                        if let Some(last) = items.last_mut() {
                            last.push(' ');
                            last.push_str(t);
                        }
                    }
                }
                i += 1;
            }
            out.push(Block::List(items));
        } else {
            para.push(l.trim().to_string());
            i += 1;
        }
    }
    flush(&mut para, &mut out);
    out
}

pub fn first_sentence(text: &str) -> String {
    let mut depth = 0;
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        match c {
            '`' => depth ^= 1,
            '{' => depth += 2,
            '}' => depth = (depth as i32 - 2).max(0) as usize,
            '.' if depth == 0 && chars.get(i + 1).map(|n| n.is_whitespace()).unwrap_or(true) => {
                return chars[..=i].iter().collect();
            }
            _ => {}
        }
    }
    text.to_string()
}

pub fn parse(raw: &str) -> DocComment {
    let lines = raw_lines(raw);
    let mut d = DocComment::default();
    let mut desc: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() && !lines[i].trim_start().starts_with('@') {
        desc.push(lines[i].clone());
        i += 1;
    }
    d.body = blocks(&desc);
    if let Some(Block::Para(p)) = d.body.first() {
        d.summary = first_sentence(p);
    }
    while i < lines.len() {
        let line = lines[i].trim_start().to_string();
        let (tag, rest) = match line.split_once(char::is_whitespace) {
            Some((t, r)) => (t.to_string(), r.trim().to_string()),
            None => (line.clone(), String::new()),
        };
        i += 1;
        let mut cont: Vec<String> = Vec::new();
        while i < lines.len() && !lines[i].trim_start().starts_with('@') {
            cont.push(lines[i].clone());
            i += 1;
        }
        let text = || {
            let mut t = rest.clone();
            for c in &cont {
                if !c.trim().is_empty() {
                    if !t.is_empty() {
                        t.push(' ');
                    }
                    t.push_str(c.trim());
                }
            }
            t
        };
        match tag.as_str() {
            "@param" => {
                let t = text();
                let (n, desc) = t.split_once(char::is_whitespace).unwrap_or((t.as_str(), ""));
                d.params.push((n.to_string(), desc.trim().to_string()));
            }
            "@return" | "@returns" => d.ret = Some(text()),
            "@throws" | "@error" => d.throws.push(text()),
            "@see" => d.see.push(text()),
            "@since" => d.since = Some(text()),
            "@deprecated" => d.deprecated = Some(text()),
            "@author" => d.authors.push(text()),
            "@group" => d.group = Some(text()),
            "@alias" => d.aliases.push(text()),
            "@example" => {
                let mut code = Vec::new();
                if !rest.is_empty() {
                    code.push(format!("    {}", rest));
                }
                code.extend(cont.iter().cloned());
                d.examples.push(dedent(&code));
            }
            _ => {
                if let Some(Block::Para(p)) = d.body.last_mut() {
                    p.push(' ');
                    p.push_str(&line);
                }
            }
        }
    }
    d
}

pub fn doc_before(src: &str, offset: usize) -> Option<String> {
    let offset = offset.min(src.len());
    let line_start = src[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let before = &src[..line_start];
    let mut lines: Vec<&str> = before.lines().collect();
    while let Some(l) = lines.last() {
        if l.trim_start().starts_with('@') {
            lines.pop();
        } else {
            break;
        }
    }
    let last = lines.last()?.trim();
    if !last.ends_with("*/") {
        return None;
    }
    let mut collected = Vec::new();
    while let Some(l) = lines.pop() {
        collected.push(l);
        let t = l.trim_start();
        if t.starts_with("/**") {
            collected.reverse();
            return Some(collected.join("\n"));
        }
        if t.starts_with("/*") || (collected.len() > 1 && t.ends_with("*/")) {
            return None;
        }
    }
    None
}

pub fn module_doc(src: &str) -> Option<String> {
    let t = src.trim_start();
    let t = if t.starts_with("#!") {
        t.split_once('\n').map(|x| x.1).unwrap_or("").trim_start()
    } else {
        t
    };
    if !t.starts_with("/**") {
        return None;
    }
    let end = t.find("*/")? + 2;
    let after = &t[end..];
    let blank = after
        .strip_prefix('\n')
        .map(|r| r.trim_start_matches([' ', '\t']).starts_with('\n'))
        .unwrap_or(false)
        || after.trim().is_empty();
    blank.then(|| t[..end].to_string())
}

pub fn inline_markdown(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("{@") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let inner = &after[..end];
        let (_, arg) = inner.split_once(char::is_whitespace).unwrap_or((inner, ""));
        out.push('`');
        out.push_str(arg.trim());
        out.push('`');
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

pub fn to_markdown(d: &DocComment) -> String {
    let mut out = String::new();
    for b in &d.body {
        match b {
            Block::Para(p) => out.push_str(&format!("{}\n\n", inline_markdown(p))),
            Block::Code(c) => out.push_str(&format!("```burn\n{}\n```\n\n", c)),
            Block::List(items) => {
                for it in items {
                    out.push_str(&format!("- {}\n", inline_markdown(it)));
                }
                out.push('\n');
            }
        }
    }
    if let Some(dep) = &d.deprecated {
        out.push_str(&format!("**Deprecated.** {}\n\n", inline_markdown(dep)));
    }
    for (n, desc) in &d.params {
        out.push_str(&format!("*@param* `{}` — {}  \n", n, inline_markdown(desc)));
    }
    if let Some(r) = &d.ret {
        out.push_str(&format!("*@return* {}  \n", inline_markdown(r)));
    }
    for t in &d.throws {
        out.push_str(&format!("*@throws* {}  \n", inline_markdown(t)));
    }
    for s in &d.see {
        out.push_str(&format!("*@see* `{}`  \n", s));
    }
    if let Some(ex) = d.examples.first() {
        out.push_str(&format!("\n```burn\n{}\n```\n", ex));
    }
    out.trim_end().to_string()
}
