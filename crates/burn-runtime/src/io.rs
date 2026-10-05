use crate::meta;
#[allow(unused_imports)]
use crate::prelude::*;

fn number_after(msg: &str, key: &str) -> Option<i64> {
    let i = msg.find(key)? + key.len();
    msg[i..].split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

fn range_help(name: &str) -> Option<String> {
    if name == "int" {
        return Some("int holds values from -9223372036854775808 to 9223372036854775807".into());
    }
    let n = meta::Num::ALL.into_iter().find(|n| n.name() == name && !n.is_float())?;
    Some(format!("{} holds values from {} to {}", name, n.min_value(), n.max_value()))
}

pub fn error_help(msg: &str) -> Option<String> {
    if let Some(rest) = msg.strip_prefix("integer overflow: ") {
        let r = range_help(rest.rsplit(' ').next()?)?;
        return Some(format!(
            "{}; use a wider type, or `wrappingAdd`, `wrappingSub` and `wrappingMul` to wrap around",
            r
        ));
    }
    if msg.starts_with("cannot convert ") && !msg.starts_with("cannot convert \"") {
        let r = range_help(msg.rsplit(' ').next()?)?;
        return Some(format!("{}; check the value before converting it with `as`", r));
    }
    let h = if msg.starts_with("index ") || msg.starts_with("string index ") {
        match number_after(msg, "(length ") {
            Some(0) => "it is empty, so there is nothing to read; check `len(...) > 0` first".to_string(),
            Some(n) => format!("valid indexes go from 0 to {}; check the index against `len(...)` first", n - 1),
            None => return None,
        }
    } else if msg == "integer overflow" {
        "the result does not fit in `int` (-9223372036854775808 to 9223372036854775807); use `float` for larger numbers".into()
    } else if msg.starts_with("cannot shift by ") {
        "the shift amount must be from 0 to 63".into()
    } else if msg == "division by zero" {
        "check that the divisor is not 0 before dividing".into()
    } else if msg == "unexpected null value" {
        "the value was null; check it with `if (x != null)` instead of using `!!`".into()
    } else if msg == "cannot index null" {
        "the value was null; check it with `if (x != null)` before indexing".into()
    } else if msg.starts_with("key ") && msg.ends_with(" not found in map") {
        "check the key with `has(map, key)` first, or pass a default: `get(map, key, fallback)`".into()
    } else if msg == "pop from empty array" {
        "check `len(items) > 0` before calling `pop`".into()
    } else if msg.starts_with("cannot convert ") {
        "the text is not a number; check it first or handle the bad input".into()
    } else if msg.starts_with("cannot cast value") {
        "check the type with `is` before casting with `as`".into()
    } else if msg == "this object was destroyed and can no longer be used" {
        "another variable or function destroyed this object; do not use it after `destroy`".into()
    } else if msg == "this object was already destroyed" {
        "each object can be destroyed only once".into()
    } else if msg == "function ended without returning a value" {
        "make sure every path through the function ends with `return`".into()
    } else if msg.starts_with("out of memory") {
        "the limit keeps the computer responsive; it is 3/4 of the memory by default, set BURN_MAX_HEAP_MB to change it (0 turns it off)".into()
    } else if msg.starts_with("stack overflow") {
        "a function probably calls itself without ever stopping; check its base case".into()
    } else {
        return None;
    };
    Some(h)
}

fn source_line(loc: &str) -> Option<(usize, usize, String)> {
    let mut parts = loc.rsplitn(3, ':');
    let col: usize = parts.next()?.parse().ok()?;
    let line: usize = parts.next()?.parse().ok()?;
    let file = parts.next()?;
    let text = read_text(file)?;
    let l = text.lines().nth(line.checked_sub(1)?)?.trim_end_matches('\r').to_string();
    Some((line, col, l))
}

pub fn format_error(msg: &str, loc: u64) -> String {
    let mut out = format!("runtime error: {}", msg);
    let help = error_help(msg);
    let Some(l) = meta::loc(loc) else {
        if let Some(h) = help {
            out.push_str(&format!("\n  = help: {}", h));
        }
        return out;
    };
    match source_line(l) {
        Some((line, col, text)) => {
            let w = line.to_string().len();
            let pad: String = text.chars().take(col.saturating_sub(1)).map(|c| if c == '\t' { '\t' } else { ' ' }).collect();
            out.push_str(&format!(
                "\n{:>w$}--> {}\n{:>w$} |\n{} | {}\n{:>w$} | {}^",
                "",
                l,
                "",
                line,
                text,
                "",
                pad,
                w = w
            ));
            if let Some(h) = help {
                out.push_str(&format!("\n{:>w$} = help: {}", "", h, w = w));
            }
        }
        None => {
            out.push_str(&format!("\n --> {}", l));
            if let Some(h) = help {
                out.push_str(&format!("\n  = help: {}", h));
            }
        }
    }
    out
}

#[cfg(not(burn_core))]
mod host {
    use super::format_error;
    use std::io::{IsTerminal, Read, Write};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};

    static OUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static CAPTURE: Mutex<Option<Vec<u8>>> = Mutex::new(None);
    static PANIC_MODE: AtomicBool = AtomicBool::new(false);
    static TTY: OnceLock<bool> = OnceLock::new();
    static ARGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

    #[derive(Debug, Clone)]
    pub struct BurnError(pub String);

    #[derive(Debug, Clone)]
    pub struct BurnExit(pub i32);

    pub fn set_panic_mode(on: bool) {
        PANIC_MODE.store(on, Ordering::SeqCst);
    }

    pub fn set_args(args: Vec<String>) {
        *ARGS.lock().unwrap_or_else(|e| e.into_inner()) = args;
    }

    pub fn args() -> Vec<String> {
        ARGS.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn start_capture() {
        *CAPTURE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Vec::new());
    }

    pub fn take_capture() -> String {
        flush();
        let mut c = CAPTURE.lock().unwrap_or_else(|e| e.into_inner());
        let v = c.take().unwrap_or_default();
        String::from_utf8_lossy(&v).into_owned()
    }

    fn is_tty() -> bool {
        *TTY.get_or_init(|| std::io::stdout().is_terminal())
    }

    pub fn write_out(b: &[u8]) {
        let mut o = OUT.lock().unwrap_or_else(|e| e.into_inner());
        o.extend_from_slice(b);
        if o.len() > 1 << 16 || (is_tty() && b.contains(&b'\n')) {
            flush_locked(&mut o);
        }
    }

    fn flush_locked(o: &mut Vec<u8>) {
        if o.is_empty() {
            return;
        }
        let mut cap = CAPTURE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = cap.as_mut() {
            c.extend_from_slice(o);
        } else {
            let out = std::io::stdout();
            let mut l = out.lock();
            let _ = l.write_all(o);
            let _ = l.flush();
        }
        o.clear();
    }

    pub fn flush_for_signal() {
        if let Ok(mut o) = OUT.try_lock() {
            if !o.is_empty() {
                let _ = std::io::stdout().write_all(&o);
                let _ = std::io::stdout().flush();
                o.clear();
            }
        }
    }

    pub fn flush() {
        let mut o = OUT.lock().unwrap_or_else(|e| e.into_inner());
        flush_locked(&mut o);
    }

    pub fn read_line(prompt: &str) -> String {
        write_out(prompt.as_bytes());
        flush();
        let mut line = String::new();
        let mut buf = [0u8; 1];
        let mut bytes = Vec::new();
        let stdin = std::io::stdin();
        let mut lock = stdin.lock();
        while let Ok(1) = lock.read(&mut buf) {
            if buf[0] == b'\n' {
                break;
            }
            bytes.push(buf[0]);
        }
        line.push_str(&String::from_utf8_lossy(&bytes));
        while line.ends_with('\r') {
            line.pop();
        }
        line
    }

    pub fn read_text(path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    pub fn write_err(b: &[u8]) {
        flush();
        let _ = std::io::stderr().write_all(b);
    }

    pub fn rt_error(msg: &str, loc: u64) -> ! {
        flush();
        let full = format_error(msg, loc);
        if PANIC_MODE.load(Ordering::SeqCst) {
            std::panic::panic_any(BurnError(full));
        }
        eprintln!("{}", full);
        std::process::exit(1)
    }

    pub fn exit_now(code: i32) -> ! {
        flush();
        if PANIC_MODE.load(Ordering::SeqCst) {
            std::panic::panic_any(BurnExit(code));
        }
        std::process::exit(code)
    }
}

#[cfg(burn_core)]
mod host {
    use super::format_error;
    use crate::prelude::*;
    use crate::sync::Global;
    use core::sync::atomic::{AtomicU8, Ordering};

    static OUT: Global<Vec<u8>> = Global::new(Vec::new());
    static ARGS: Global<Vec<String>> = Global::new(Vec::new());
    static TTY: AtomicU8 = AtomicU8::new(2);

    pub fn read_text(path: &str) -> Option<String> {
        crate::sys::read_text(path)
    }

    pub fn set_args(args: Vec<String>) {
        ARGS.with(|a| *a = args);
    }

    pub fn args() -> Vec<String> {
        ARGS.with(|a| a.clone())
    }

    fn is_tty() -> bool {
        if TTY.load(Ordering::Relaxed) == 2 {
            TTY.store(crate::sys::stdout_is_tty() as u8, Ordering::Relaxed);
        }
        TTY.load(Ordering::Relaxed) == 1
    }

    pub fn write_out(b: &[u8]) {
        let tty = is_tty();
        OUT.with(|o| {
            o.extend_from_slice(b);
            if o.len() > 1 << 16 || (tty && b.contains(&b'\n')) {
                crate::sys::write_fd(1, o);
                o.clear();
            }
        });
    }

    pub fn flush() {
        OUT.with(|o| {
            crate::sys::write_fd(1, o);
            o.clear();
        });
    }

    pub fn flush_for_signal() {
        flush();
    }

    pub fn write_err(b: &[u8]) {
        flush();
        crate::sys::write_fd(2, b);
    }

    pub fn read_line(prompt: &str) -> String {
        write_out(prompt.as_bytes());
        flush();
        let mut bytes = Vec::new();
        while let Some(b) = crate::sys::read_byte() {
            if b == b'\n' {
                break;
            }
            bytes.push(b);
        }
        while bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    pub fn rt_error(msg: &str, loc: u64) -> ! {
        flush();
        let mut full = format_error(msg, loc);
        full.push('\n');
        crate::sys::write_fd(2, full.as_bytes());
        crate::sys::exit_process(1)
    }

    pub fn exit_now(code: i32) -> ! {
        flush();
        crate::sys::exit_process(code)
    }
}

pub use host::*;
