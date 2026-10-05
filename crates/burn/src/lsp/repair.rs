use crate::lexer::{self, Tok};

fn indent_at(src: &str, offset: usize) -> usize {
    let start = src[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    src[start..]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

pub fn close_braces(src: &str) -> Option<(String, Vec<usize>)> {
    let (tokens, _) = lexer::lex(src, 0);
    let mut open: Vec<usize> = Vec::new();
    let mut inserts: Vec<usize> = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        match t.kind {
            Tok::LBrace => open.push(indent_at(src, t.span.start as usize)),
            Tok::RBrace => {
                let first_on_line = i == 0 || t.nl_before;
                if first_on_line {
                    let ind = indent_at(src, t.span.start as usize);
                    while open.len() > 1 && open.last().map(|o| *o > ind).unwrap_or(false) {
                        open.pop();
                        inserts.push(t.span.start as usize);
                    }
                }
                open.pop();
            }
            _ => {}
        }
    }
    let tail = open.len();
    if inserts.is_empty() && tail == 0 {
        return None;
    }
    let mut out = String::with_capacity(src.len() + inserts.len() + tail * 2);
    let mut last = 0;
    for at in &inserts {
        out.push_str(&src[last..*at]);
        out.push('}');
        last = *at;
    }
    out.push_str(&src[last..]);
    for _ in 0..tail {
        out.push_str("\n}");
    }
    Some((out, inserts))
}

pub fn shift(inserts: &[usize], offset: usize) -> usize {
    offset + inserts.iter().filter(|i| **i <= offset).count()
}

pub fn unshift(inserts: &[usize], offset: usize) -> usize {
    let mut o = offset;
    for (n, i) in inserts.iter().enumerate() {
        if offset > i + n {
            o -= 1;
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_an_open_block_before_the_outer_brace() {
        let src = "fun main() {\n    if x {\n        y\n}\n";
        let (out, ins) = close_braces(src).unwrap();
        assert_eq!(out, "fun main() {\n    if x {\n        y\n}}\n");
        assert_eq!(ins.len(), 1);
    }

    #[test]
    fn closes_at_the_end_of_the_file() {
        let src = "fun main() {\n    var k = 1\n";
        let (out, ins) = close_braces(src).unwrap();
        assert_eq!(out, "fun main() {\n    var k = 1\n\n}");
        assert!(ins.is_empty());
    }

    #[test]
    fn leaves_balanced_code_alone() {
        assert!(close_braces("fun main() {\n    if x {\n    }\n}\n").is_none());
    }
}
