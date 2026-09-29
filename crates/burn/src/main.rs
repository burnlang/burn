mod ast;
mod check;
mod diag;
mod driver;
mod hir;
mod lexer;
mod native;
mod loader;
mod parser;
mod source;
mod types;
mod vm;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match driver::compile_path(std::path::Path::new(&args[1])) {
        Ok(c) => {
            driver::report(&c.sm, &c.warnings);
            if args.len() > 2 && args[2] == "hir" {
                for f in &c.program.funcs {
                    println!("{} {:?}", f.name, f.body);
                }
            }
            if args.len() > 2 && args[2] == "native" {
                let o = native::BuildOptions { output: "a.out".into(), emit_asm: Some("a.s".into()), strip: true };
                if let Err(e) = native::build(&c.program, &o) { eprintln!("{}", e); }
                return;
            }
            std::process::exit(vm::exec::run_program(&c.program, vec![]));
        }
        Err(f) => driver::report(&f.sm, &f.diags),
    }
}
