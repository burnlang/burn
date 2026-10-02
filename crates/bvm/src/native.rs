use crate::archive::{self, Archive};
use crate::exec::{load_with, needs_link, Host, Program, Vm};
use crate::link::{export_name, link, rebase};
use crate::module::Module;
use burn_runtime::meta::{self, Meta};
use burn_runtime::{api, io};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::sync::{Arc, Mutex};

struct Lib {
    key: usize,
    prog: Arc<Program>,
    globals: usize,
    funcs: HashMap<String, u32>,
}

static LIBS: Mutex<Vec<Arc<Lib>>> = Mutex::new(Vec::new());
static EXPORTS: Mutex<Vec<(String, u64, u32)>> = Mutex::new(Vec::new());
static MIXINS: Mutex<String> = Mutex::new(String::new());

struct PooledVm(Box<Vm>);

thread_local! {
    static POOL: RefCell<HashMap<usize, Vec<PooledVm>>> = RefCell::new(HashMap::new());
}

fn cstr(p: u64) -> String {
    if p == 0 {
        return String::new();
    }
    unsafe { CStr::from_ptr(p as usize as *const std::ffi::c_char) }.to_string_lossy().into_owned()
}

#[no_mangle]
pub extern "C" fn burn_bvm_register_exports(table: u64, n: u64) -> u64 {
    let mut ex = EXPORTS.lock().unwrap_or_else(|e| e.into_inner());
    for i in 0..n as usize {
        let e = unsafe { std::slice::from_raw_parts((table as usize as *const u64).add(3 * i), 3) };
        ex.push((cstr(e[0]), e[1], e[2] as u32));
    }
    0
}

#[no_mangle]
pub extern "C" fn burn_bvm_register_mixins(text: u64) -> u64 {
    *MIXINS.lock().unwrap_or_else(|e| e.into_inner()) = cstr(text);
    0
}

fn with_native_mixins(lib: Module) -> Result<Module, String> {
    let text = MIXINS.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if text.trim().is_empty() {
        return Ok(lib);
    }
    let mut mixins = crate::asm::assemble(&text).map_err(|e| format!("native mixins: {}", e))?;
    let has = |name: &str| {
        lib.funcs.iter().enumerate().any(|(i, f)| {
            !f.external
                && (f.name == name
                    || export_name(&lib, i as u32).as_deref() == Some(name)
                    || (!lib.name.is_empty() && format!("{}::{}", lib.name, f.name) == name))
        })
    };
    mixins.annotations.retain(|a| match a.arg("target").and_then(|v| v.as_str()) {
        Some(t) => has(t),
        None => false,
    });
    if mixins.annotations.is_empty() {
        return Ok(lib);
    }
    link(&[lib, mixins])
}

fn call_native(fnptr: u64, args: &[u64]) -> u64 {
    let tramp = api::trampoline();
    if tramp == 0 {
        io::rt_error("native functions can only be called from a native executable", u64::MAX);
    }
    let rev: Vec<u64> = args.iter().rev().copied().collect();
    let t: extern "C" fn(u64, u64, u64) -> u64 = unsafe { std::mem::transmute(tramp) };
    t(fnptr, rev.as_ptr() as u64, rev.len() as u64)
}

fn native_host(extra: Option<Host>) -> Host {
    let mut host = extra.unwrap_or_default();
    for (name, fnptr, argc) in EXPORTS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        let f = *fnptr;
        host.register(name, *argc, move |a| call_native(f, a));
    }
    host
}

fn prepare(bytes: &[u8]) -> Result<(Module, Option<Host>), String> {
    if archive::is_archive(bytes) {
        let a = Archive::decode(bytes)?;
        return Ok((a.link()?, Some(a.host())));
    }
    Ok((crate::parse(bytes)?, None))
}

fn load_lib(key: usize, bytes: &[u8]) -> Result<Arc<Lib>, String> {
    let (m, extra) = prepare(bytes)?;
    let m = if needs_link(&m) { link(std::slice::from_ref(&m))? } else { m };
    let m = with_native_mixins(m)?;
    let base = meta::meta();
    let rebased = rebase(&m, &base.types, base.locs.len() as u32)?;
    let mut locs = base.locs.clone();
    locs.extend(m.locs.iter().cloned());
    meta::set_meta(Meta {
        types: rebased.types.clone(),
        locs,
        info: Vec::new(),
    });
    let host = native_host(extra);
    let prog = load_with(&rebased, &host, false).map_err(|e| e.to_string())?;
    let globals: &'static mut [u64] = Box::leak(vec![0u64; prog.nglobals.max(1)].into_boxed_slice());
    let mut funcs = HashMap::new();
    for (i, f) in rebased.funcs.iter().enumerate() {
        funcs.entry(f.name.clone()).or_insert(i as u32);
        if let Some(e) = export_name(&rebased, i as u32) {
            funcs.insert(e, i as u32);
        }
    }
    Ok(Arc::new(Lib {
        key,
        prog,
        globals: globals.as_mut_ptr() as usize,
        funcs,
    }))
}

fn with_vm<R>(lib: &Lib, f: impl FnOnce(&mut Vm) -> R) -> R {
    let pooled = POOL.with(|p| p.borrow_mut().get_mut(&lib.key).and_then(|v| v.pop()));
    let mut vm = pooled.unwrap_or_else(|| PooledVm(Vm::new(lib.prog.clone(), lib.globals as *mut u64)));
    let r = f(&mut vm.0);
    POOL.with(|p| p.borrow_mut().entry(lib.key).or_default().push(vm));
    r
}

fn get_lib(blob: u64, len: u64) -> Arc<Lib> {
    let key = blob as usize;
    {
        let libs = LIBS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(l) = libs.iter().find(|l| l.key == key) {
            return l.clone();
        }
    }
    let bytes = unsafe { std::slice::from_raw_parts(blob as usize as *const u8, len as usize) };
    let lib = match load_lib(key, bytes) {
        Ok(l) => l,
        Err(e) => io::rt_error(&format!("could not load an embedded bytecode library: {}", e), u64::MAX),
    };
    LIBS.lock().unwrap_or_else(|e| e.into_inner()).push(lib.clone());
    if let Some(e) = lib.prog.entry {
        with_vm(&lib, |vm| vm.call(e, &[]));
    }
    lib
}

#[no_mangle]
pub extern "C" fn burn_bvm_init_lib(blob: u64, len: u64) -> u64 {
    get_lib(blob, len);
    0
}

#[no_mangle]
pub extern "C" fn burn_bvm_call(blob: u64, len: u64, name: u64, argc: u64, argv_rev: u64) -> u64 {
    let lib = get_lib(blob, len);
    let name = cstr(name);
    let Some(f) = lib.funcs.get(&name).copied() else {
        io::rt_error(&format!("the bytecode library has no function {}", name), u64::MAX);
    };
    let params = lib.prog.funcs[f as usize].params as u64;
    if params != argc {
        io::rt_error(&format!("{} takes {} arguments but was called with {}", name, params, argc), u64::MAX);
    }
    let args: Vec<u64> = (0..argc as usize).rev().map(|i| unsafe { *(argv_rev as usize as *const u64).add(i) }).collect();
    with_vm(&lib, |vm| vm.call(f, &args))
}

#[no_mangle]
pub extern "C" fn burn_bvm_main(blob: u64, len: u64, argc: u64, argv: u64) -> u64 {
    let mut args = Vec::new();
    for i in 1..argc as usize {
        let p = unsafe { *(argv as usize as *const u64).add(i) };
        args.push(cstr(p));
    }
    let bytes = unsafe { std::slice::from_raw_parts(blob as usize as *const u8, len as usize) };
    let result = crate::load_bytes(bytes).and_then(|(m, host)| {
        if m.entry.is_none() {
            return Err("the bundled program has no entry function".to_string());
        }
        crate::run(&m, &host, args).map_err(|e| e.to_string())
    });
    match result {
        Ok(code) => code as u64,
        Err(e) => {
            eprintln!("error: {}", e);
            1
        }
    }
}
