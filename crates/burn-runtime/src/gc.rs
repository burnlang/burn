use crate::fx::FxBuild;
use crate::obj::*;
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

const DEFAULT_THRESHOLD: usize = 32 * 1024 * 1024;

fn min_threshold() -> usize {
    static T: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *T.get_or_init(|| {
        std::env::var("BURN_GC_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_THRESHOLD)
    })
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
    static M: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *M.get_or_init(|| match std::env::var("BURN_MAX_HEAP_MB").ok().and_then(|v| v.trim().parse::<usize>().ok()) {
        Some(0) => usize::MAX,
        Some(mb) => mb.saturating_mul(1 << 20),
        None => physical_memory().map(|m| m / 4 * 3).unwrap_or(usize::MAX),
    })
}

struct Heap {
    objs: HashSet<usize, FxBuild>,
    live: usize,
    threshold: usize,
    stack_base: usize,
    ranges: Vec<(usize, usize)>,
    vm_stacks: Vec<usize>,
    main_thread: Option<std::thread::ThreadId>,
    collections: usize,
}

static HEAP: Mutex<Heap> = Mutex::new(Heap {
    objs: HashSet::with_hasher(FxBuild),
    live: 0,
    threshold: 0,
    stack_base: 0,
    ranges: Vec::new(),
    vm_stacks: Vec::new(),
    main_thread: None,
    collections: 0,
});

static SINCE: AtomicUsize = AtomicUsize::new(0);
pub static TASKS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static TEMP: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
    static IS_MAIN: Cell<bool> = const { Cell::new(false) };
}

fn heap() -> MutexGuard<'static, Heap> {
    HEAP.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn set_stack_base(base: usize) {
    let mut h = heap();
    h.stack_base = base;
    h.main_thread = Some(std::thread::current().id());
    IS_MAIN.with(|m| m.set(true));
}

pub fn clear_stack_base() {
    heap().stack_base = 0;
}

pub fn add_root_range(start: usize, words: usize) {
    heap().ranges.push((start, words));
}

pub fn clear_root_ranges() {
    heap().ranges.clear();
}

pub fn add_vm_stack(v: *const Vec<u64>) {
    heap().vm_stacks.push(v as usize);
}

pub fn remove_vm_stack(v: *const Vec<u64>) {
    let mut h = heap();
    if let Some(i) = h.vm_stacks.iter().position(|x| *x == v as usize) {
        h.vm_stacks.swap_remove(i);
    }
}

pub fn collections() -> usize {
    heap().collections
}

pub fn live_objects() -> usize {
    heap().objs.len()
}

#[inline]
pub fn account(bytes: usize) {
    SINCE.fetch_add(bytes, Ordering::Relaxed);
}

pub struct RootGuard;

impl Drop for RootGuard {
    fn drop(&mut self) {
        TEMP.with(|t| {
            t.borrow_mut().pop();
        });
    }
}

pub fn root(v: u64) -> RootGuard {
    TEMP.with(|t| t.borrow_mut().push(v));
    RootGuard
}

pub fn alloc(kind: u8, tid: u32, size: usize) -> u64 {
    let size = (size + 15) & !15;
    let mut h = heap();
    if h.threshold == 0 {
        h.threshold = min_threshold();
    }
    if SINCE.load(Ordering::Relaxed) >= h.threshold && h.stack_base != 0 && TASKS.load(Ordering::SeqCst) == 0 && IS_MAIN.with(|m| m.get()) {
        collect(&mut h);
    }
    let layout = Layout::from_size_align(size, 16).unwrap();
    let p = unsafe { alloc_zeroed(layout) };
    if p.is_null() {
        std::alloc::handle_alloc_error(layout);
    }
    unsafe {
        let hd = &mut *(p as *mut Header);
        hd.kind = kind;
        hd.tid = tid;
        hd.size = size as u64;
    }
    h.objs.insert(p as usize);
    drop(h);
    SINCE.fetch_add(size, Ordering::Relaxed);
    p as u64
}

pub fn force_collect() {
    let mut h = heap();
    if h.stack_base != 0 && TASKS.load(Ordering::SeqCst) == 0 && IS_MAIN.with(|m| m.get()) {
        collect(&mut h);
    }
}

#[inline(always)]
fn spill(regs: &mut [usize; 16]) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        std::arch::asm!(
            "mov [{r}], rbx",
            "mov [{r} + 8], rbp",
            "mov [{r} + 16], r12",
            "mov [{r} + 24], r13",
            "mov [{r} + 32], r14",
            "mov [{r} + 40], r15",
            r = in(reg) regs.as_mut_ptr(),
            options(nostack, preserves_flags)
        );
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        std::arch::asm!(
            "stp x19, x20, [{r}]",
            "stp x21, x22, [{r}, #16]",
            "stp x23, x24, [{r}, #32]",
            "stp x25, x26, [{r}, #48]",
            "stp x27, x28, [{r}, #64]",
            "str x29, [{r}, #80]",
            r = in(reg) regs.as_mut_ptr(),
            options(nostack, preserves_flags)
        );
    }
    std::hint::black_box(&regs);
}

#[inline]
fn consider(h: &Heap, w: usize, work: &mut Vec<usize>) {
    if w & 15 != 0 || w == 0 {
        return;
    }
    if h.objs.contains(&w) {
        let hd = unsafe { &mut *(w as *mut Header) };
        if hd.mark == 0 {
            hd.mark = 1;
            work.push(w);
        }
    }
}

fn scan_words(h: &Heap, start: usize, end: usize, work: &mut Vec<usize>) {
    let mut p = (start + 7) & !7;
    while p + 8 <= end {
        let w = unsafe { std::ptr::read_volatile(p as *const usize) };
        consider(h, w, work);
        p += 8;
    }
}

#[inline(never)]
fn collect(h: &mut Heap) {
    let mut regs = [0usize; 16];
    spill(&mut regs);
    let sp = regs.as_ptr() as usize;
    let mut work: Vec<usize> = Vec::with_capacity(1024);
    for r in regs.iter() {
        consider(h, *r, &mut work);
    }
    if h.stack_base > sp {
        scan_words(h, sp, h.stack_base, &mut work);
    }
    for (start, words) in h.ranges.clone() {
        scan_words(h, start, start + words * 8, &mut work);
    }
    for v in h.vm_stacks.clone() {
        let vec = unsafe { &*(v as *const Vec<u64>) };
        for w in vec.iter() {
            consider(h, *w as usize, &mut work);
        }
    }
    TEMP.with(|t| {
        for w in t.borrow().iter() {
            consider(h, *w as usize, &mut work);
        }
    });
    while let Some(p) = work.pop() {
        let v = p as u64;
        match kind_of(v) {
            K_ARRAY => {
                for w in array_slice(v) {
                    consider(h, *w as usize, &mut work);
                }
            }
            K_STRUCT => {
                for i in 0..struct_len(v) {
                    consider(h, field(v, i) as usize, &mut work);
                }
            }
            K_BOX => consider(h, box_val(v) as usize, &mut work),
            K_MAP => {
                let d = map_data(v);
                for w in d.keys.iter().chain(d.vals.iter()) {
                    consider(h, *w as usize, &mut work);
                }
            }
            K_FUTURE => {
                let st = future_state(v);
                let val = *st.value.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(x) = val {
                    consider(h, x as usize, &mut work);
                }
            }
            _ => {}
        }
    }
    let mut live = 0usize;
    h.objs.retain(|&p| {
        let hd = unsafe { &mut *(p as *mut Header) };
        if hd.mark != 0 {
            hd.mark = 0;
            live += hd.size as usize;
            if hd.kind == K_ARRAY {
                live += unsafe { word(p as u64, ARR_CAP) as usize * 8 };
            }
            true
        } else {
            unsafe { free_obj(p) };
            false
        }
    });
    h.live = live;
    let max = max_heap();
    let mut threshold = min_threshold().max(live);
    if max != usize::MAX {
        threshold = threshold.min(max.saturating_sub(live).max(1 << 20));
    }
    h.threshold = threshold;
    h.collections += 1;
    SINCE.store(0, Ordering::Relaxed);
    if std::env::var_os("BURN_GC_STATS").is_some() {
        eprintln!("[gc] collection {} live objects {} live bytes {}", h.collections, h.objs.len(), live);
    }
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

unsafe fn free_obj(p: usize) {
    let v = p as u64;
    let hd = &*(p as *const Header);
    match hd.kind {
        K_ARRAY => array_free_data(v),
        K_MAP => map_free(v),
        K_FUTURE => future_free(v),
        _ => {}
    }
    let size = hd.size as usize;
    dealloc(p as *mut u8, Layout::from_size_align(size, 16).unwrap());
}
