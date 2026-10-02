use crate::api::{b, fv};
use crate::io::{self, rt_error};
use crate::obj::{array_of_strings, str_bytes, str_ref, string};
use crate::prelude::*;
use crate::sys;

fn unavailable(what: &str) -> ! {
    rt_error(&format!("{} needs the standard runtime, which this program was built without", what), u64::MAX)
}

pub fn print_err(s: u64) -> u64 {
    let mut v = Vec::with_capacity(str_bytes(s).len() + 1);
    v.extend_from_slice(str_bytes(s));
    v.push(b'\n');
    io::write_err(&v);
    0
}

pub fn read_stdin() -> u64 {
    io::flush();
    string(&String::from_utf8_lossy(&sys::read_all(0)))
}

pub fn now_ms() -> u64 {
    sys::wall_ns() / 1_000_000
}

pub fn now_sec() -> u64 {
    fv(sys::wall_ns() as f64 / 1e9)
}

pub fn clock_ns() -> u64 {
    use core::sync::atomic::{AtomicU64, Ordering};
    static START: AtomicU64 = AtomicU64::new(0);
    let now = sys::mono_ns();
    if START.load(Ordering::Relaxed) == 0 {
        START.store(now.max(1), Ordering::Relaxed);
    }
    now.saturating_sub(START.load(Ordering::Relaxed))
}

pub fn sleep_ms(ms: u64) -> u64 {
    io::flush();
    sys::sleep_ms((ms as i64).max(0) as u64);
    0
}

pub fn read_file(path: u64, loc: u64) -> u64 {
    match sys::read_file(str_ref(path)) {
        Some(b) => string(&String::from_utf8_lossy(&b)),
        None => rt_error(&format!("cannot read file \"{}\"", str_ref(path)), loc),
    }
}

pub fn write_file(path: u64, content: u64) -> u64 {
    b(sys::write_file(str_ref(path), str_bytes(content), false))
}

pub fn append_file(path: u64, content: u64) -> u64 {
    b(sys::write_file(str_ref(path), str_bytes(content), true))
}

pub fn file_exists(path: u64) -> u64 {
    b(sys::exists(str_ref(path)))
}

pub fn env_var(name: u64) -> u64 {
    string(&sys::env(str_ref(name)).unwrap_or_default())
}

pub fn exit_now(code: u64) -> u64 {
    io::exit_now(code as i32)
}

pub fn rt_exit(code: u64) -> u64 {
    io::flush();
    code
}

pub fn local_time() -> u64 {
    unavailable("localTime")
}

pub fn http_request(_method: u64, _url: u64, _body: u64, _headers: u64) -> u64 {
    unavailable("HTTP")
}

pub fn json_parse(_s: u64, _loc: u64) -> u64 {
    unavailable("JSON")
}

pub fn json_stringify(_v: u64, _tid: u64) -> u64 {
    unavailable("JSON")
}

pub fn exec(_program: u64, _args: u64, _capture: u64) -> u64 {
    unavailable("running processes")
}

pub fn fs_op(_op: u64, _a: u64, _c: u64) -> u64 {
    unavailable("file system operations")
}

pub fn list_dir(_path: u64) -> u64 {
    array_of_strings(&[])
}

pub fn cwd() -> u64 {
    unavailable("the working directory")
}

pub fn spawn_native(_fnptr: u64, _argc: u64, _argsptr: u64, _tid: u64) -> u64 {
    unavailable("async")
}

pub fn await_future(_fut: u64) -> u64 {
    unavailable("async")
}
