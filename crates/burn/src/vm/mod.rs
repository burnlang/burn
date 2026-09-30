pub mod compile;

use crate::check::libs::library_modules;
use crate::hir::Program;
use burn_runtime::io;
use bvm::{Host, LoadError, Module, Runner};
use std::sync::Arc;

pub fn module(p: &Program) -> Module {
    compile::compile(p)
}

pub fn library_parts(p: &Program) -> Result<(Vec<Module>, Host), String> {
    let mut mods = Vec::new();
    let mut host = Host::new();
    for l in &p.libs {
        mods.extend(library_modules(&l.bytes).map_err(|e| format!("{}: {}", l.path.display(), e))?);
        if bvm::archive::is_archive(&l.bytes) {
            let a = bvm::archive::Archive::decode(&l.bytes).map_err(|e| format!("{}: {}", l.path.display(), e))?;
            host.extend(&a.host());
        }
    }
    Ok((mods, host))
}

pub fn linked(p: &Program) -> Result<(Module, Host), String> {
    let m = module(p);
    if p.libs.is_empty() {
        return Ok((m, Host::new()));
    }
    let (libs, host) = library_parts(p)?;
    let mut mods = vec![m];
    mods.extend(libs);
    Ok((bvm::link(&mods)?, host))
}

pub fn prepare(p: &Program) -> Arc<bvm::Program> {
    let (m, host) = match linked(p) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1)
        }
    };
    match bvm::load(&m, &host) {
        Ok(prog) => prog,
        Err(e @ (LoadError::Link(_) | LoadError::MissingImport(_) | LoadError::ImportArity { .. } | LoadError::Unlinked(_))) => {
            eprintln!("error: {}", e);
            std::process::exit(1)
        }
        Err(e) => {
            eprintln!("internal error: the compiler produced an invalid bvm module: {}", e);
            std::process::exit(70)
        }
    }
}

pub fn run_program(p: &Program, args: Vec<String>) -> i32 {
    io::set_args(args);
    let prog = prepare(p);
    let mut r = Runner::new(prog.clone(), Vec::new());
    if let Some(e) = prog.entry {
        r.call(e);
    }
    r.finish();
    0
}
