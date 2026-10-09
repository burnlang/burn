use crate::obj::*;
use crate::rc;
use std::sync::atomic::Ordering;
use std::sync::{Condvar, Mutex};

static DONE_LOCK: Mutex<()> = Mutex::new(());
static DONE_CV: Condvar = Condvar::new();

pub fn spawn(tid: u32, job: Box<dyn FnOnce() -> u64 + Send>) -> u64 {
    rc::task_started();
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
        rc::task_finished();
        DONE_CV.notify_all();
    });
    if r.is_err() {
        rc::task_finished();
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
    while rc::TASKS.load(Ordering::SeqCst) > 0 {
        l = DONE_CV.wait(l).unwrap_or_else(|e| e.into_inner());
    }
}
