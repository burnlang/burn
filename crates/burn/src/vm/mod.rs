pub mod compile;

use crate::hir::Program;
use burn_runtime::io;
use bvm::{Host, Module, Runner};
use std::sync::Arc;

pub fn module(p: &Program) -> Module {
    compile::compile(p)
}

pub fn prepare(p: &Program) -> Arc<bvm::Program> {
    let m = module(p);
    match bvm::load(&m, &Host::new()) {
        Ok(prog) => prog,
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
