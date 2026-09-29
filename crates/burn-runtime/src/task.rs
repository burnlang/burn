use crate::gc;
use crate::obj::*;
use std::sync::atomic::Ordering;
use std::sync::{Condvar, Mutex};

static DONE_LOCK: Mutex<()> = Mutex::new(());
static DONE_CV: Condvar = Condvar::new();

pub fn spawn(tid: u32, job: Box<dyn FnOnce() -> u64 + Send>) -> u64 {
    gc::TASKS.fetch_add(1, Ordering::SeqCst);
    let fut = future_new(tid);
    let st = future_state(fut);
    let r = std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(move || {
        let v = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)) {
            Ok(v) => v,
            Err(e) => {
                if let Some(err) = e.downcast_ref::<crate::io::BurnError>() {
                    eprintln!("{}", err.0);
                }
                0
            }
        };
        {
            let mut g = st.value.lock().unwrap_or_else(|e| e.into_inner());
            *g = Some(v);
        }
        st.cv.notify_all();
        let _l = DONE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        gc::TASKS.fetch_sub(1, Ordering::SeqCst);
        DONE_CV.notify_all();
    });
    if r.is_err() {
        gc::TASKS.fetch_sub(1, Ordering::SeqCst);
        crate::io::rt_error("could not spawn async task", u64::MAX);
    }
    fut
}

pub fn await_future(fut: u64) -> u64 {
    let st = future_state(fut);
    let mut g = st.value.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if let Some(v) = *g {
            return v;
        }
        g = st.cv.wait(g).unwrap_or_else(|e| e.into_inner());
    }
}

pub fn wait_all() {
    let mut l = DONE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    while gc::TASKS.load(Ordering::SeqCst) > 0 {
        l = DONE_CV.wait(l).unwrap_or_else(|e| e.into_inner());
    }
}
