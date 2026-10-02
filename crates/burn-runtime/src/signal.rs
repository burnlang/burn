#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod imp {
    use core::sync::atomic::{AtomicUsize, Ordering};

    #[repr(C)]
    struct SigAction {
        handler: usize,
        mask: [u64; 16],
        flags: i32,
        restorer: usize,
    }

    #[repr(C)]
    struct StackT {
        sp: *mut u8,
        flags: i32,
        size: usize,
    }

    extern "C" {
        fn sigaction(sig: i32, act: *const SigAction, old: *mut SigAction) -> i32;
        fn sigaltstack(ss: *const StackT, old: *mut StackT) -> i32;
        fn write(fd: i32, buf: *const u8, n: usize) -> isize;
        fn _exit(code: i32) -> !;
    }

    const SIGSEGV: i32 = 11;
    const SIGBUS: i32 = 7;
    const SA_SIGINFO: i32 = 4;
    const SA_ONSTACK: i32 = 0x0800_0000;
    const ALT_STACK: usize = 64 * 1024;

    static STACK_BASE: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn handler(_sig: i32, info: *const u8, _ctx: *const u8) {
        let addr = *(info.add(16) as *const usize);
        let base = STACK_BASE.load(Ordering::Relaxed);
        let near_stack = base != 0 && addr < base && base - addr < 1 << 30;
        let msg: &[u8] = if near_stack {
            b"runtime error: stack overflow (recursion is too deep)\n"
        } else {
            b"runtime error: segmentation fault (this is a bug in Burn, please report it)\n"
        };
        crate::io::flush_for_signal();
        write(2, msg.as_ptr(), msg.len());
        _exit(1);
    }

    pub fn install(stack_base: usize) {
        STACK_BASE.store(stack_base, Ordering::Relaxed);
        unsafe {
            let mem = alloc::alloc::alloc(alloc::alloc::Layout::from_size_align(ALT_STACK, 16).unwrap());
            if mem.is_null() {
                return;
            }
            let ss = StackT {
                sp: mem,
                flags: 0,
                size: ALT_STACK,
            };
            if sigaltstack(&ss, core::ptr::null_mut()) != 0 {
                return;
            }
            let act = SigAction {
                handler: handler as *const () as usize,
                mask: [0; 16],
                flags: SA_SIGINFO | SA_ONSTACK,
                restorer: 0,
            };
            sigaction(SIGSEGV, &act, core::ptr::null_mut());
            sigaction(SIGBUS, &act, core::ptr::null_mut());
        }
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
mod imp {
    pub fn install(_stack_base: usize) {}
}

pub use imp::install;
