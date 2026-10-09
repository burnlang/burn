use crate::prelude::*;

#[cfg(not(burn_core))]
pub struct Global<T>(std::sync::Mutex<T>);

#[cfg(not(burn_core))]
impl<T> Global<T> {
    pub const fn new(v: T) -> Global<T> {
        Global(std::sync::Mutex::new(v))
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

#[cfg(burn_core)]
pub struct Global<T>(core::cell::UnsafeCell<T>);

#[cfg(burn_core)]
unsafe impl<T> Sync for Global<T> {}

#[cfg(burn_core)]
impl<T> Global<T> {
    pub const fn new(v: T) -> Global<T> {
        Global(core::cell::UnsafeCell::new(v))
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(unsafe { &mut *self.0.get() })
    }
}

#[cfg(burn_core)]
pub struct Local<T>(T);

#[cfg(burn_core)]
unsafe impl<T> Sync for Local<T> {}

#[cfg(burn_core)]
impl<T> Local<T> {
    pub const fn new(v: T) -> Local<T> {
        Local(v)
    }

    pub fn with<R>(&'static self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.0)
    }
}

pub fn env(name: &str) -> Option<String> {
    #[cfg(not(burn_core))]
    return std::env::var(name).ok();
    #[cfg(burn_core)]
    return crate::sys::env(name);
}

pub fn cached(slot: &core::sync::atomic::AtomicUsize, f: impl FnOnce() -> usize) -> usize {
    use core::sync::atomic::Ordering;
    let v = slot.load(Ordering::Relaxed);
    if v != 0 {
        return v - 1;
    }
    let v = f().min(usize::MAX - 1);
    slot.store(v + 1, Ordering::Relaxed);
    v
}
