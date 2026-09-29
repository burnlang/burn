use crate::meta;
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
    loop {
        match lock.read(&mut buf) {
            Ok(1) => {
                if buf[0] == b'\n' {
                    break;
                }
                bytes.push(buf[0]);
            }
            _ => break,
        }
    }
    line.push_str(&String::from_utf8_lossy(&bytes));
    while line.ends_with('\r') {
        line.pop();
    }
    line
}

pub fn format_error(msg: &str, loc: u64) -> String {
    match meta::loc(loc) {
        Some(l) => format!("runtime error: {}\n  --> {}", msg, l),
        None => format!("runtime error: {}", msg),
    }
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
