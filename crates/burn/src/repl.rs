use crate::check::CheckOptions;
use crate::diag::{render, Severity};
use crate::driver;
use crate::loader::Loader;
use crate::vm::prepare;
use bvm::runtime::io::{self, BurnError, BurnExit};
use bvm::Runner;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};
use std::process::ExitCode;

fn balance(src: &str) -> i32 {
    let mut depth = 0i32;
    let mut in_str: Option<char> = None;
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match in_str {
            Some(q) => {
                if c == '\\' {
                    chars.next();
                } else if c == q {
                    in_str = None;
                }
            }
            None => match c {
                '"' | '\'' => in_str = Some(c),
                '/' if chars.peek() == Some(&'/') => {
                    for n in chars.by_ref() {
                        if n == '\n' {
                            break;
                        }
                    }
                }
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' => depth -= 1,
                _ => {}
            },
        }
    }
    depth
}

pub fn install_quiet_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let p = info.payload();
        if p.downcast_ref::<BurnError>().is_some() || p.downcast_ref::<BurnExit>().is_some() {
            return;
        }
        default(info);
    }));
}

pub struct Session {
    source: String,
    saved: HashMap<(String, String), u64>,
    inited: HashSet<String>,
}

pub enum Outcome {
    Ok,
    CompileError(String),
    RuntimeError(String),
    Exit(i32),
}

impl Session {
    pub fn new() -> Session {
        Session {
            source: String::new(),
            saved: HashMap::new(),
            inited: HashSet::new(),
        }
    }

    pub fn eval(&mut self, input: &str) -> Outcome {
        let offset = self.source.len() + 1;
        let candidate = format!("{}\n{}\n", self.source, input);
        let mut loader = Loader::new();
        let root = loader.load_source("repl", candidate.clone(), std::env::current_dir().ok());
        let loaded = loader.finish(root);
        let root_file = loaded.modules[root].file;
        let opts = CheckOptions {
            skip_before: Some((root_file, offset as u32)),
            want_index: false,
            repl_echo: true,
        };
        let compiled = match driver::check_loaded(loaded, opts) {
            Ok(c) => c,
            Err(f) => {
                let mut msg = String::new();
                for d in f
                    .diags
                    .iter()
                    .filter(|d| d.span.file != root_file || d.span.start as usize >= offset || d.severity == Severity::Error)
                {
                    if d.span.file == root_file && (d.span.start as usize) < offset {
                        continue;
                    }
                    msg.push_str(&render(&f.sm, d, driver::use_color()));
                }
                return Outcome::CompileError(msg);
            }
        };
        let p = &compiled.program;
        let code = prepare(p);
        let globals: Vec<u64> = p
            .globals
            .iter()
            .map(|g| *self.saved.get(&(g.module.clone(), g.name.clone())).unwrap_or(&0))
            .collect();
        let mut runner = Runner::new(code, globals);
        let root_key = p.inits.last().map(|x| x.0.clone()).unwrap_or_default();
        let mut result = Outcome::Ok;
        for (key, init) in &p.inits {
            if *key != root_key && self.inited.contains(key) {
                continue;
            }
            let init = *init;
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runner.call(init)));
            io::flush();
            if let Err(e) = r {
                if let Some(err) = e.downcast_ref::<BurnError>() {
                    result = Outcome::RuntimeError(err.0.clone());
                } else if let Some(ex) = e.downcast_ref::<BurnExit>() {
                    result = Outcome::Exit(ex.0);
                } else {
                    result = Outcome::RuntimeError("internal error".into());
                }
                break;
            }
            if *key != root_key {
                self.inited.insert(key.clone());
            }
        }
        let globals = runner.finish();
        for (i, g) in p.globals.iter().enumerate() {
            if let Some(v) = globals.get(i) {
                self.saved.insert((g.module.clone(), g.name.clone()), *v);
            }
        }
        if !matches!(result, Outcome::CompileError(_)) {
            self.source = candidate.trim_end().to_string();
        }
        if !compiled.warnings.is_empty() {
            for d in compiled.warnings.iter().filter(|d| d.span.file == root_file && d.span.start as usize >= offset) {
                eprint!("{}", render(&compiled.sm, d, driver::use_color()));
            }
        }
        result
    }
}

pub fn run() -> ExitCode {
    io::set_panic_mode(true);
    install_quiet_hook();
    println!("Burn {} REPL. Type :help for help, :quit to exit.", env!("CARGO_PKG_VERSION"));
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let mut session = Session::new();
    loop {
        print!(">>> ");
        let _ = std::io::stdout().flush();
        let mut input = match lines.next() {
            Some(Ok(l)) => l,
            _ => break,
        };
        while balance(&input) > 0 {
            print!("... ");
            let _ = std::io::stdout().flush();
            match lines.next() {
                Some(Ok(l)) => {
                    input.push('\n');
                    input.push_str(&l);
                }
                _ => break,
            }
        }
        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }
        match trimmed {
            ":q" | ":quit" | ":exit" | "exit" | "quit" => break,
            ":help" | ":h" => {
                println!("Enter Burn statements, declarations or expressions. Expression results are printed.");
                println!("  :quit    leave the REPL");
                println!("  :reset   forget everything defined so far");
                println!("  :source  show the code entered so far");
                continue;
            }
            ":reset" => {
                session = Session::new();
                println!("session cleared");
                continue;
            }
            ":source" => {
                println!("{}", session.source.trim());
                continue;
            }
            _ => {}
        }
        match session.eval(&input) {
            Outcome::Ok => {}
            Outcome::CompileError(m) => eprint!("{}", m),
            Outcome::RuntimeError(m) => eprintln!("{}", m),
            Outcome::Exit(code) => return ExitCode::from(code as u8),
        }
    }
    ExitCode::SUCCESS
}
