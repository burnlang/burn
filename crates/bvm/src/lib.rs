pub mod asm;
pub mod binary;
pub mod builder;
pub mod exec;
pub mod link;
pub mod mixin;
pub mod module;
pub mod op;
pub mod verify;

pub use builder::{FuncBuilder, FuncId, Label, ModuleBuilder};
pub use burn_runtime as runtime;
pub use burn_runtime::meta::Desc;
pub use burn_runtime::RtFn;
pub use exec::{load, run, Host, LoadError, Program, Runner};
pub use link::{link, link_with, LinkOptions};
pub use module::{Annotation, Function, Import, Module, Sig, Table, Target, Value};
pub use op::{Cmp, Op};
pub use verify::{verify, VerifyError};

pub const FORMAT_VERSION: u16 = 2;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn parse(bytes: &[u8]) -> Result<Module, String> {
    if binary::is_binary(bytes) {
        return binary::decode(bytes);
    }
    let src = std::str::from_utf8(bytes).map_err(|_| "the file is neither bytecode nor UTF-8 assembly".to_string())?;
    asm::assemble(src).map_err(|e| e.to_string())
}

pub fn read(path: &std::path::Path) -> Result<Module, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("could not read {}: {}", path.display(), e))?;
    parse(&bytes).map_err(|e| format!("{}: {}", path.display(), e))
}
