use crate::prelude::*;
use core::alloc::{GlobalAlloc, Layout};

extern "C" {
    fn malloc(size: usize) -> *mut u8;
    fn calloc(n: usize, size: usize) -> *mut u8;
    fn realloc(p: *mut u8, size: usize) -> *mut u8;
    fn free(p: *mut u8);
    fn posix_memalign(out: *mut *mut u8, align: usize, size: usize) -> i32;
    fn write(fd: i32, buf: *const u8, n: usize) -> isize;
    fn read(fd: i32, buf: *mut u8, n: usize) -> isize;
    fn open(path: *const u8, flags: i32, ...) -> i32;
    fn close(fd: i32) -> i32;
    fn exit(code: i32) -> !;
    fn abort() -> !;
    fn getenv(name: *const u8) -> *const u8;
    fn strlen(s: *const u8) -> usize;
    fn isatty(fd: i32) -> i32;
    fn access(path: *const u8, mode: i32) -> i32;
    fn clock_gettime(clock: i32, ts: *mut Timespec) -> i32;
    fn nanosleep(req: *const Timespec, rem: *mut Timespec) -> i32;
}

#[repr(C)]
struct Timespec {
    sec: i64,
    nsec: i64,
}

#[cfg(target_os = "linux")]
const O_WRONLY_CREAT: i32 = 0o1 | 0o100;
#[cfg(target_os = "linux")]
const O_TRUNC: i32 = 0o1000;
#[cfg(target_os = "linux")]
const O_APPEND: i32 = 0o2000;
#[cfg(target_os = "linux")]
const CLOCK_MONOTONIC: i32 = 1;

#[cfg(not(target_os = "linux"))]
const O_WRONLY_CREAT: i32 = 0x1 | 0x200;
#[cfg(not(target_os = "linux"))]
const O_TRUNC: i32 = 0x400;
#[cfg(not(target_os = "linux"))]
const O_APPEND: i32 = 0x8;
#[cfg(not(target_os = "linux"))]
const CLOCK_MONOTONIC: i32 = 6;

struct Malloc;

unsafe impl GlobalAlloc for Malloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if l.align() <= 16 {
            malloc(l.size().max(1))
        } else {
            let mut p = core::ptr::null_mut();
            if posix_memalign(&mut p, l.align(), l.size()) != 0 {
                return core::ptr::null_mut();
            }
            p
        }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        if l.align() <= 16 {
            calloc(1, l.size().max(1))
        } else {
            let p = self.alloc(l);
            if !p.is_null() {
                core::ptr::write_bytes(p, 0, l.size());
            }
            p
        }
    }
    unsafe fn dealloc(&self, p: *mut u8, _l: Layout) {
        free(p)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        if l.align() <= 16 {
            realloc(p, new.max(1))
        } else {
            let q = self.alloc(Layout::from_size_align_unchecked(new, l.align()));
            if !q.is_null() {
                core::ptr::copy_nonoverlapping(p, q, l.size().min(new));
                free(p);
            }
            q
        }
    }
}

#[global_allocator]
static ALLOC: Malloc = Malloc;

#[panic_handler]
fn on_panic(_info: &core::panic::PanicInfo) -> ! {
    let msg = b"runtime error: internal error in the Burn runtime\n";
    unsafe {
        write(2, msg.as_ptr(), msg.len());
        abort()
    }
}

pub fn write_fd(fd: i32, mut b: &[u8]) {
    while !b.is_empty() {
        let n = unsafe { write(fd, b.as_ptr(), b.len()) };
        if n <= 0 {
            return;
        }
        b = &b[n as usize..];
    }
}

pub fn exit_process(code: i32) -> ! {
    unsafe { exit(code) }
}

pub fn env(name: &str) -> Option<String> {
    let mut c = Vec::with_capacity(name.len() + 1);
    c.extend_from_slice(name.as_bytes());
    c.push(0);
    unsafe {
        let p = getenv(c.as_ptr());
        if p.is_null() {
            return None;
        }
        let s = core::slice::from_raw_parts(p, strlen(p));
        Some(String::from_utf8_lossy(s).into_owned())
    }
}

fn c_path(path: &str) -> Vec<u8> {
    let mut c = Vec::with_capacity(path.len() + 1);
    c.extend_from_slice(path.as_bytes());
    c.push(0);
    c
}

pub fn read_all(fd: i32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = unsafe { read(fd, buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            break;
        }
        out.extend_from_slice(&buf[..n as usize]);
    }
    out
}

pub fn read_byte() -> Option<u8> {
    let mut b = 0u8;
    if unsafe { read(0, &mut b, 1) } == 1 {
        Some(b)
    } else {
        None
    }
}

pub fn read_file(path: &str) -> Option<Vec<u8>> {
    let c = c_path(path);
    let fd = unsafe { open(c.as_ptr(), 0) };
    if fd < 0 {
        return None;
    }
    let out = read_all(fd);
    unsafe { close(fd) };
    Some(out)
}

pub fn read_text(path: &str) -> Option<String> {
    read_file(path).map(|b| String::from_utf8_lossy(&b).into_owned())
}

pub fn write_file(path: &str, data: &[u8], append: bool) -> bool {
    let c = c_path(path);
    let flags = O_WRONLY_CREAT | if append { O_APPEND } else { O_TRUNC };
    let fd = unsafe { open(c.as_ptr(), flags, 0o644) };
    if fd < 0 {
        return false;
    }
    let mut rest = data;
    let mut ok = true;
    while !rest.is_empty() {
        let n = unsafe { write(fd, rest.as_ptr(), rest.len()) };
        if n <= 0 {
            ok = false;
            break;
        }
        rest = &rest[n as usize..];
    }
    unsafe { close(fd) };
    ok
}

pub fn exists(path: &str) -> bool {
    let c = c_path(path);
    unsafe { access(c.as_ptr(), 0) == 0 }
}

fn clock(id: i32) -> u64 {
    let mut ts = Timespec { sec: 0, nsec: 0 };
    unsafe { clock_gettime(id, &mut ts) };
    (ts.sec as u64).wrapping_mul(1_000_000_000).wrapping_add(ts.nsec as u64)
}

pub fn wall_ns() -> u64 {
    clock(0)
}

pub fn mono_ns() -> u64 {
    clock(CLOCK_MONOTONIC)
}

pub fn sleep_ms(ms: u64) {
    let ts = Timespec {
        sec: (ms / 1000) as i64,
        nsec: ((ms % 1000) * 1_000_000) as i64,
    };
    unsafe { nanosleep(&ts, core::ptr::null_mut()) };
}

pub fn stdout_is_tty() -> bool {
    unsafe { isatty(1) == 1 }
}

#[no_mangle]
pub extern "C" fn rust_eh_personality() {}
