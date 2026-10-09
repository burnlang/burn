use crate::module::{Module, Target, Value};
use crate::op::{rt_by_name, Op};
use bvm_runtime::RtFn;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const SOURCE: &str = include_str!("../runtime.bvm");
pub const MARKER: &str = "RefCounted";

pub struct Written {
    pub module: Module,
    pub funcs: HashMap<RtFn, u32>,
}

pub fn written() -> &'static Written {
    static W: OnceLock<Written> = OnceLock::new();
    W.get_or_init(|| {
        let module = crate::asm::assemble(SOURCE).unwrap_or_else(|e| panic!("bvm/runtime.bvm does not assemble: {}", e));
        let mut funcs = HashMap::new();
        for a in &module.annotations {
            let Target::Func(f) = a.target else { continue };
            if a.name != "Runtime" {
                continue;
            }
            let Some((_, Value::Str(name))) = a.args.iter().find(|(k, _)| k == "name") else {
                continue;
            };
            let r = rt_by_name(name).unwrap_or_else(|| panic!("bvm/runtime.bvm implements unknown runtime function `{}`", name));
            let params = module.funcs[f as usize].params as usize;
            assert_eq!(
                params,
                r.argc(),
                "`{}` in bvm/runtime.bvm takes {} arguments instead of {}",
                name,
                params,
                r.argc()
            );
            funcs.insert(r, f);
        }
        Written { module, funcs }
    })
}

fn is_burn(m: &Module) -> bool {
    m.annotations.iter().any(|a| a.target == Target::Module && a.name == MARKER)
}

fn calls_written(m: &Module, w: &Written) -> bool {
    m.funcs
        .iter()
        .any(|f| f.code.iter().any(|op| matches!(op, Op::Rt(r) if w.funcs.contains_key(r))))
}

pub fn link(m: &Module) -> Result<Option<Module>, String> {
    if !is_burn(m) {
        return Ok(None);
    }
    let w = written();
    if !calls_written(m, w) {
        return Ok(None);
    }
    let base = m.funcs.iter().filter(|f| !f.external).count() as u32;
    let mut rank = vec![u32::MAX; w.module.funcs.len()];
    let mut next = 0;
    for (i, f) in w.module.funcs.iter().enumerate() {
        if !f.external {
            rank[i] = base + next;
            next += 1;
        }
    }
    let mut out = crate::link::link(&[m.clone(), w.module.clone()])?;
    out.annotations
        .retain(|a| !(a.name == "Runtime" && matches!(a.target, Target::Func(f) if f >= base)));
    let calls: HashMap<RtFn, u32> = w.funcs.iter().map(|(r, f)| (*r, rank[*f as usize])).collect();
    for f in out.funcs.iter_mut() {
        for op in f.code.iter_mut() {
            if let Op::Rt(r) = *op {
                if let Some(target) = calls.get(&r) {
                    *op = Op::Call(*target);
                }
            }
        }
    }
    Ok(Some(out))
}
