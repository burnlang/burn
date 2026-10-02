use crate::meta::{self, desc, Desc, I_BOX_TRACK, I_TRACK};
use crate::obj::*;
use crate::prelude::*;
use crate::sync::{cached, env, Global};
use alloc::alloc::{alloc_zeroed, dealloc, Layout};
use core::cell::{Cell, RefCell};
use core::sync::atomic::{AtomicU32, AtomicU8, AtomicUsize, Ordering};

const DEFAULT_THRESHOLD: usize = 32 * 1024 * 1024;
const ROOTS_LIMIT: usize = 20_000;

const BLACK: u8 = 0;
const GRAY: u8 = 1;
const WHITE: u8 = 2;
const PURPLE: u8 = 3;

#[no_mangle]
pub static burn_rc_mt: AtomicU8 = AtomicU8::new(0);

pub static TASKS: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static SINCE: AtomicUsize = AtomicUsize::new(0);
static COLLECTIONS: AtomicUsize = AtomicUsize::new(0);
static ZOMBIES: Global<Vec<usize>> = Global::new(Vec::new());

#[cfg(not(burn_core))]
thread_local! {
    static ROOTS: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
    static IS_MAIN: Cell<bool> = const { Cell::new(false) };
    static COLLECTING: Cell<bool> = const { Cell::new(false) };
}

#[cfg(burn_core)]
static ROOTS: crate::sync::Local<RefCell<Vec<u64>>> = crate::sync::Local::new(RefCell::new(Vec::new()));
#[cfg(burn_core)]
static IS_MAIN: crate::sync::Local<Cell<bool>> = crate::sync::Local::new(Cell::new(false));
#[cfg(burn_core)]
static COLLECTING: crate::sync::Local<Cell<bool>> = crate::sync::Local::new(Cell::new(false));

fn threshold() -> usize {
    static T: AtomicUsize = AtomicUsize::new(0);
    cached(&T, || env("BURN_GC_THRESHOLD").and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_THRESHOLD))
}

pub const K_FREED: u8 = 0xEE;

fn checking() -> bool {
    static C: AtomicUsize = AtomicUsize::new(0);
    cached(&C, || env("BURN_RC_CHECK").is_some() as usize) == 1
}

#[cold]
fn bad_object(p: u64, what: &str) -> ! {
    crate::io::rt_error(&format!("internal error: {} of a freed object at {:#x}", what, p), u64::MAX)
}

fn stats() -> bool {
    static S: AtomicUsize = AtomicUsize::new(0);
    cached(&S, || env("BURN_GC_STATS").is_some() as usize) == 1
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn physical_memory() -> Option<usize> {
    extern "C" {
        fn sysconf(name: i32) -> i64;
    }
    #[cfg(target_os = "linux")]
    let (pages, size) = (85, 30);
    #[cfg(target_os = "macos")]
    let (pages, size) = (200, 29);
    let (n, sz) = unsafe { (sysconf(pages), sysconf(size)) };
    if n <= 0 || sz <= 0 {
        return None;
    }
    Some((n as usize).saturating_mul(sz as usize))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn physical_memory() -> Option<usize> {
    None
}

pub fn max_heap() -> usize {
    static M: AtomicUsize = AtomicUsize::new(0);
    cached(&M, || match env("BURN_MAX_HEAP_MB").and_then(|v| v.trim().parse::<usize>().ok()) {
        Some(0) => usize::MAX,
        Some(mb) => mb.saturating_mul(1 << 20),
        None => physical_memory().map(|m| m / 4 * 3).unwrap_or(usize::MAX),
    })
}

pub fn set_main_thread() {
    IS_MAIN.with(|m| m.set(true));
}

pub fn collections() -> usize {
    COLLECTIONS.load(Ordering::Relaxed)
}

pub fn live_bytes() -> usize {
    LIVE.load(Ordering::Relaxed)
}

#[inline]
fn multi() -> bool {
    burn_rc_mt.load(Ordering::Relaxed) != 0
}

pub fn task_started() {
    TASKS.fetch_add(1, Ordering::SeqCst);
    burn_rc_mt.store(1, Ordering::SeqCst);
}

pub fn task_finished() {
    if TASKS.fetch_sub(1, Ordering::SeqCst) == 1 {
        burn_rc_mt.store(0, Ordering::SeqCst);
    }
}

#[inline]
pub fn account(bytes: usize) {
    LIVE.fetch_add(bytes, Ordering::Relaxed);
    SINCE.fetch_add(bytes, Ordering::Relaxed);
}

#[inline]
pub fn unaccount(bytes: usize) {
    LIVE.fetch_sub(bytes, Ordering::Relaxed);
}

pub fn alloc(kind: u8, tid: u32, size: usize) -> u64 {
    let size = (size + 15) & !15;
    if size > u32::MAX as usize {
        crate::io::rt_error("out of memory: a single value is larger than 4 GB", u64::MAX);
    }
    if SINCE.load(Ordering::Relaxed) >= threshold() || LIVE.load(Ordering::Relaxed) > max_heap() {
        safe_point();
    }
    let layout = Layout::from_size_align(size, 16).unwrap();
    let p = unsafe { alloc_zeroed(layout) };
    if p.is_null() {
        alloc::alloc::handle_alloc_error(layout);
    }
    let track = match kind {
        K_STR => 0,
        K_BOX => meta::info(tid) & I_BOX_TRACK,
        _ => meta::info(tid) & I_TRACK,
    };
    unsafe {
        let hd = &mut *(p as *mut Header);
        hd.kind = kind;
        hd.tid = tid;
        hd.size = size as u32;
        hd.rc = 1;
        if track != 0 {
            hd.flags |= F_TRACK;
        }
    }
    account(size);
    p as u64
}

fn safe_point() {
    if multi() || !IS_MAIN.with(|m| m.get()) || COLLECTING.with(|c| c.get()) {
        return;
    }
    drain_zombies();
    let roots = ROOTS.with(|r| r.borrow().len());
    if roots > 0 {
        collect_cycles();
    }
    SINCE.store(0, Ordering::Relaxed);
    let live = LIVE.load(Ordering::Relaxed);
    let max = max_heap();
    if live > max {
        crate::io::rt_error(
            &format!(
                "out of memory: the program keeps {} MB of data alive, more than its limit of {} MB",
                live >> 20,
                max >> 20
            ),
            u64::MAX,
        );
    }
}

#[inline]
fn rc_cell<'a>(p: u64) -> &'a AtomicU32 {
    unsafe { AtomicU32::from_ptr((p as usize + 12) as *mut u32) }
}

#[inline]
pub fn retain(p: u64) -> u64 {
    if p == 0 {
        return 0;
    }
    let h = unsafe { hdr(p) };
    if h.flags & F_STATIC != 0 {
        return p;
    }
    if h.kind == K_FREED {
        bad_object(p, "retain");
    }
    if multi() {
        rc_cell(p).fetch_add(1, Ordering::Relaxed);
    } else {
        h.rc = h.rc.wrapping_add(1);
    }
    p
}

#[inline]
pub fn retain_t(v: u64, tid: u32) -> u64 {
    if meta::managed(tid) {
        retain(v);
    }
    v
}

#[inline]
pub fn release_t(v: u64, tid: u32) {
    if meta::managed(tid) {
        release(v);
    }
}

pub fn release(p: u64) {
    if p == 0 {
        return;
    }
    let h = unsafe { hdr(p) };
    if h.flags & F_STATIC != 0 {
        return;
    }
    if h.kind == K_FREED {
        bad_object(p, "release");
    }
    if multi() {
        if rc_cell(p).fetch_sub(1, Ordering::AcqRel) == 1 {
            zombie(p);
        }
        return;
    }
    h.rc = h.rc.wrapping_sub(1);
    if h.rc == 0 {
        release_zero(p);
    } else {
        possible_root(p);
    }
}

pub fn zero_reached(p: u64) {
    release_zero(p);
}

pub fn possible_root(p: u64) {
    let h = unsafe { hdr(p) };
    if h.flags & F_TRACK == 0 || multi() {
        return;
    }
    if h.mark != PURPLE {
        h.mark = PURPLE;
        if h.flags & F_BUFFERED == 0 {
            h.flags |= F_BUFFERED;
            ROOTS.with(|r| r.borrow_mut().push(p));
        }
    }
    if IS_MAIN.with(|m| m.get()) && ROOTS.with(|r| r.borrow().len()) >= ROOTS_LIMIT {
        SINCE.store(threshold(), Ordering::Relaxed);
    }
}

fn zombie(p: u64) {
    let h = unsafe { hdr(p) };
    ZOMBIES.with(|z| {
        if h.flags & F_ZOMBIE == 0 {
            h.flags |= F_ZOMBIE;
            z.push(p as usize);
        }
    })
}

pub fn drain_zombies() {
    if multi() {
        return;
    }
    let list = ZOMBIES.with(core::mem::take);
    for p in list {
        let h = unsafe { hdr(p as u64) };
        h.flags &= !F_ZOMBIE;
        if h.rc == 0 {
            release_zero(p as u64);
        }
    }
}

pub fn for_children(p: u64, mut f: impl FnMut(u64)) {
    let h = unsafe { hdr(p) };
    match h.kind {
        K_ARRAY => {
            if let Desc::Array(e) = desc(h.tid) {
                if meta::managed(*e) {
                    for w in array_slice(p) {
                        if *w != 0 {
                            f(*w);
                        }
                    }
                }
            }
        }
        K_STRUCT => {
            if let Desc::Record { fields, .. } = desc(h.tid) {
                let n = struct_len(p).min(fields.len());
                for (i, fd) in fields.iter().enumerate().take(n) {
                    if meta::managed(fd.1) {
                        let w = field(p, i);
                        if w != 0 {
                            f(w);
                        }
                    }
                }
            }
        }
        K_BOX => {
            if meta::managed(h.tid) {
                let w = box_val(p);
                if w != 0 {
                    f(w);
                }
            }
        }
        K_MAP => {
            if let Desc::Map(k, v) = desc(h.tid) {
                let d = map_data(p);
                if meta::managed(*k) {
                    for w in d.keys.iter() {
                        if *w != 0 {
                            f(*w);
                        }
                    }
                }
                if meta::managed(*v) {
                    for w in d.vals.iter() {
                        if *w != 0 {
                            f(*w);
                        }
                    }
                }
            }
        }
        #[cfg(not(burn_core))]
        K_FUTURE => {
            if let Desc::Future(t) = desc(h.tid) {
                if meta::managed(*t) {
                    let st = future_state(p);
                    let val = *st.value.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(w) = val {
                        if w != 0 {
                            f(w);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn is_static(p: u64) -> bool {
    unsafe { hdr(p).flags & F_STATIC != 0 }
}

fn release_zero(p: u64) {
    let mut work = vec![p];
    while let Some(o) = work.pop() {
        for_children(o, |c| {
            if is_static(c) {
                return;
            }
            let ch = unsafe { hdr(c) };
            if ch.kind == K_FREED {
                bad_object(c, "release");
            }
            ch.rc = ch.rc.wrapping_sub(1);
            if ch.rc == 0 {
                work.push(c);
            } else {
                possible_root(c);
            }
        });
        let h = unsafe { hdr(o) };
        h.mark = BLACK;
        if h.flags & F_BUFFERED == 0 {
            unsafe { free_obj(o) };
        }
    }
}

pub unsafe fn free_obj(p: u64) {
    let hd = &*(p as usize as *const Header);
    match hd.kind {
        K_ARRAY => {
            unaccount(word(p, ARR_CAP) as usize * 8);
            array_free_data(p);
        }
        K_MAP => map_free(p),
        #[cfg(not(burn_core))]
        K_FUTURE => future_free(p),
        _ => {}
    }
    let size = hd.size as usize;
    unaccount(size);
    if checking() {
        let h = hdr(p);
        h.kind = K_FREED;
        h.rc = 0xDEAD;
        core::ptr::write_bytes((p as usize + HDR) as *mut u8, 0xEE, size - HDR);
        return;
    }
    dealloc(p as usize as *mut u8, Layout::from_size_align(size, 16).unwrap());
}

pub fn force_collect() {
    if multi() || !IS_MAIN.with(|m| m.get()) {
        return;
    }
    drain_zombies();
    collect_cycles();
}

fn color(p: u64) -> u8 {
    unsafe { hdr(p).mark }
}

fn set_color(p: u64, c: u8) {
    unsafe { hdr(p).mark = c }
}

fn mark_gray(p: u64) {
    let mut stack = vec![p];
    while let Some(s) = stack.pop() {
        if color(s) == GRAY {
            continue;
        }
        set_color(s, GRAY);
        for_children(s, |c| {
            if is_static(c) {
                return;
            }
            let h = unsafe { hdr(c) };
            h.rc = h.rc.wrapping_sub(1);
            if h.mark != GRAY {
                stack.push(c);
            }
        });
    }
}

fn scan_black(p: u64) {
    let mut stack = vec![p];
    set_color(p, BLACK);
    while let Some(s) = stack.pop() {
        for_children(s, |c| {
            if is_static(c) {
                return;
            }
            let h = unsafe { hdr(c) };
            h.rc = h.rc.wrapping_add(1);
            if h.mark != BLACK {
                h.mark = BLACK;
                stack.push(c);
            }
        });
    }
}

fn scan(p: u64) {
    let mut stack = vec![p];
    while let Some(s) = stack.pop() {
        if color(s) != GRAY {
            continue;
        }
        if unsafe { hdr(s).rc } > 0 {
            scan_black(s);
        } else {
            set_color(s, WHITE);
            for_children(s, |c| {
                if !is_static(c) && color(c) == GRAY {
                    stack.push(c);
                }
            });
        }
    }
}

fn collect_white(p: u64, garbage: &mut Vec<u64>) {
    let mut stack = vec![p];
    while let Some(s) = stack.pop() {
        let h = unsafe { hdr(s) };
        if h.mark != WHITE || h.flags & F_BUFFERED != 0 {
            continue;
        }
        h.mark = BLACK;
        for_children(s, |c| {
            if !is_static(c) && color(c) == WHITE {
                stack.push(c);
            }
        });
        garbage.push(s);
    }
}

pub fn collect_cycles() {
    if COLLECTING.with(|c| c.replace(true)) {
        return;
    }
    let roots = ROOTS.with(|r| core::mem::take(&mut *r.borrow_mut()));
    let mut kept = Vec::with_capacity(roots.len());
    for s in roots {
        let h = unsafe { hdr(s) };
        if h.mark == PURPLE && h.rc > 0 {
            mark_gray(s);
            kept.push(s);
        } else {
            h.flags &= !F_BUFFERED;
            if h.mark == BLACK && h.rc == 0 {
                unsafe { free_obj(s) };
            }
        }
    }
    for s in &kept {
        scan(*s);
    }
    let mut garbage = Vec::new();
    for s in &kept {
        unsafe { hdr(*s).flags &= !F_BUFFERED };
        collect_white(*s, &mut garbage);
    }
    let freed = garbage.len();
    for g in garbage {
        unsafe { free_obj(g) };
    }
    COLLECTIONS.fetch_add(1, Ordering::Relaxed);
    if stats() {
        crate::io::write_err(format!("[gc] cycle collection {} freed {} objects, {} bytes live\n", collections(), freed, live_bytes()).as_bytes());
    }
    COLLECTING.with(|c| c.set(false));
}
