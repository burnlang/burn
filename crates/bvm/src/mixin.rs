use crate::module::{Annotation, Module, Target, Value};
use crate::op::{rt_by_name, Op};

pub const KINDS: [&str; 3] = ["Inject", "Overwrite", "Redirect"];

#[derive(Clone, Debug)]
struct Mixin {
    hook: u32,
    target: u32,
    kind: Kind,
    priority: i64,
    order: usize,
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Overwrite,
    Head { cancellable: bool },
    Return,
    RedirectCall(u32),
    RedirectRt(Op),
    RedirectHost(u32),
}

fn find_target(m: &Module, hook: u32, name: &str) -> Result<u32, String> {
    let hook_name = &m.funcs[hook as usize].name;
    let found: Vec<u32> = (0..m.funcs.len() as u32)
        .filter(|i| {
            let f = &m.funcs[*i as usize];
            f.name == name || (!m.name.is_empty() && format!("{}::{}", m.name, f.name) == name)
        })
        .collect();
    match found.as_slice() {
        [t] => {
            if m.funcs[*t as usize].external {
                return Err(format!(
                    "mixin {} targets {}, which is native or external code; mixins can only change bytecode",
                    hook_name, name
                ));
            }
            if *t == hook {
                return Err(format!("mixin {} targets itself", hook_name));
            }
            Ok(*t)
        }
        [] => {
            if m.imports.iter().any(|i| i.name == name) {
                return Err(format!(
                    "mixin {} targets {}, which is a native host function; mixins can only change bytecode",
                    hook_name, name
                ));
            }
            Err(format!("mixin {} targets {}, but no function has that name", hook_name, name))
        }
        _ => Err(format!("mixin {} targets {}, but several functions have that name", hook_name, name)),
    }
}

fn str_arg<'a>(a: &'a Annotation, key: &str) -> Option<&'a str> {
    a.arg(key).and_then(|v| v.as_str())
}

fn parse(m: &Module) -> Result<Vec<Mixin>, String> {
    let mut out = Vec::new();
    for (order, a) in m.annotations.iter().enumerate() {
        if !KINDS.contains(&a.name.as_str()) {
            continue;
        }
        let Target::Func(hook) = a.target else {
            return Err(format!("@{} must be attached to a function", a.name));
        };
        let hook_name = m.funcs[hook as usize].name.clone();
        let tname = str_arg(a, "target")
            .or_else(|| str_arg(a, "value"))
            .ok_or_else(|| format!("@{} on {} needs target=\"function\"", a.name, hook_name))?;
        let target = find_target(m, hook, tname)?;
        let hp = m.funcs[hook as usize].params;
        let tp = m.funcs[target as usize].params;
        let priority = match a.arg("priority") {
            Some(Value::Int(p)) => *p,
            None => 1000,
            Some(_) => return Err(format!("@{} on {}: priority must be an integer", a.name, hook_name)),
        };
        let kind = match a.name.as_str() {
            "Overwrite" => {
                if hp != tp {
                    return Err(format!("@Overwrite: {} takes {} parameters but {} takes {}", hook_name, hp, tname, tp));
                }
                Kind::Overwrite
            }
            "Inject" => match str_arg(a, "at").unwrap_or("head") {
                "head" | "HEAD" => {
                    if hp != tp {
                        return Err(format!("@Inject at head: {} takes {} parameters but {} takes {}", hook_name, hp, tname, tp));
                    }
                    Kind::Head {
                        cancellable: a.arg("cancellable").and_then(|v| v.as_bool()).unwrap_or(false),
                    }
                }
                "return" | "RETURN" | "tail" | "TAIL" => {
                    if hp != tp && hp != tp + 1 {
                        return Err(format!(
                            "@Inject at return: {} must take the {} parameters of {} (and optionally its result)",
                            hook_name, tp, tname
                        ));
                    }
                    Kind::Return
                }
                other => return Err(format!("@Inject on {}: unknown position at=\"{}\" (use head or return)", hook_name, other)),
            },
            _ => {
                if let Some(call) = str_arg(a, "call") {
                    let callee = find_target(m, hook, call).or_else(|_| {
                        m.funcs
                            .iter()
                            .position(|f| f.name == call)
                            .map(|i| i as u32)
                            .ok_or_else(|| format!("@Redirect on {}: no function named {}", hook_name, call))
                    })?;
                    let cp = m.funcs[callee as usize].params;
                    if cp != hp {
                        return Err(format!("@Redirect: {} takes {} parameters but {} takes {}", hook_name, hp, call, cp));
                    }
                    Kind::RedirectCall(callee)
                } else if let Some(rt) = str_arg(a, "rt") {
                    let f = rt_by_name(rt).ok_or_else(|| format!("@Redirect on {}: unknown runtime function {}", hook_name, rt))?;
                    if f.argc() as u32 != hp {
                        return Err(format!("@Redirect: {} takes {} parameters but rt {} takes {}", hook_name, hp, rt, f.argc()));
                    }
                    Kind::RedirectRt(Op::Rt(f))
                } else if let Some(h) = str_arg(a, "host") {
                    let i = m
                        .imports
                        .iter()
                        .position(|x| x.name == h)
                        .ok_or_else(|| format!("@Redirect on {}: no import named {}", hook_name, h))?;
                    if m.imports[i].argc != hp {
                        return Err(format!(
                            "@Redirect: {} takes {} parameters but {} takes {}",
                            hook_name, hp, h, m.imports[i].argc
                        ));
                    }
                    Kind::RedirectHost(i as u32)
                } else {
                    return Err(format!("@Redirect on {} needs call=\"function\", rt=\"name\" or host=\"name\"", hook_name));
                }
            }
        };
        out.push(Mixin {
            hook,
            target,
            kind,
            priority,
            order,
        });
    }
    Ok(out)
}

fn shift(op: Op, by: u32) -> Op {
    match op.jump_target() {
        Some(t) => op.with_jump_target(t + by),
        None => op,
    }
}

fn expand(code: &[Op], mut f: impl FnMut(Op) -> Option<Vec<Op>>) -> Vec<Op> {
    let mut starts = Vec::with_capacity(code.len() + 1);
    let mut parts: Vec<Vec<Op>> = Vec::with_capacity(code.len());
    let mut pos = 0u32;
    for op in code {
        starts.push(pos);
        let part = f(*op).unwrap_or_else(|| vec![*op]);
        pos += part.len() as u32;
        parts.push(part);
    }
    starts.push(pos);
    let mut out = Vec::with_capacity(pos as usize);
    for (i, part) in parts.into_iter().enumerate() {
        let single = part.len() == 1 && part[0] == code[i];
        for op in part {
            out.push(match op.jump_target() {
                Some(t) if single => op.with_jump_target(starts[t as usize]),
                _ => op,
            });
        }
    }
    out
}

fn args(n: u32) -> Vec<Op> {
    (0..n).map(Op::Load).collect()
}

pub fn apply(m: &mut Module) -> Result<Vec<String>, String> {
    let mut mixins = parse(m)?;
    if mixins.is_empty() {
        return Ok(Vec::new());
    }
    mixins.sort_by_key(|x| {
        let rank = match x.kind {
            Kind::Overwrite => 0,
            Kind::RedirectCall(_) | Kind::RedirectRt(_) | Kind::RedirectHost(_) => 1,
            Kind::Return => 2,
            Kind::Head { .. } => 3,
        };
        let order = if rank == 3 { -(x.order as i64) } else { x.order as i64 };
        (x.target, rank, if rank == 3 { -x.priority } else { x.priority }, order)
    });
    let mut overwritten: Vec<u32> = Vec::new();
    let mut log = Vec::new();
    for x in &mixins {
        let hook_name = m.funcs[x.hook as usize].name.clone();
        let t = &mut m.funcs[x.target as usize];
        let tname = t.name.clone();
        let p = t.params;
        let what = match &x.kind {
            Kind::Overwrite => {
                if overwritten.contains(&x.target) {
                    return Err(format!("{} is overwritten by more than one mixin", tname));
                }
                overwritten.push(x.target);
                let mut code = args(p);
                code.push(Op::Call(x.hook));
                code.push(Op::Ret);
                t.code = code;
                "Overwrite"
            }
            Kind::Head { cancellable } => {
                let mut pro = args(p);
                pro.push(Op::Call(x.hook));
                if *cancellable {
                    let base = pro.len() as u32;
                    pro.extend([Op::Dup, Op::Jz(base + 3), Op::Ret, Op::Pop]);
                } else {
                    pro.push(Op::Pop);
                }
                let by = pro.len() as u32;
                pro.extend(t.code.iter().map(|op| shift(*op, by)));
                t.code = pro;
                "Inject at head"
            }
            Kind::Return => {
                let with_result = m.funcs[x.hook as usize].params == p + 1;
                let t = &mut m.funcs[x.target as usize];
                let tmp = t.locals;
                t.locals += 1;
                if !t.names.is_empty() {
                    t.names.push(format!("mixin{}", tmp));
                }
                let hook = x.hook;
                t.code = expand(&t.code, |op| {
                    let result: Vec<Op> = match op {
                        Op::Ret => vec![Op::Store(tmp)],
                        Op::RetVoid => vec![Op::Const(0), Op::Store(tmp)],
                        _ => return None,
                    };
                    let mut seq = result;
                    seq.extend(args(p));
                    if with_result {
                        seq.push(Op::Load(tmp));
                        seq.push(Op::Call(hook));
                        seq.push(Op::Ret);
                    } else {
                        seq.push(Op::Call(hook));
                        seq.push(Op::Pop);
                        seq.push(Op::Load(tmp));
                        seq.push(Op::Ret);
                    }
                    Some(seq)
                });
                "Inject at return"
            }
            Kind::RedirectCall(callee) => {
                let n = redirect(&mut t.code, Op::Call(*callee), x.hook);
                if n == 0 {
                    return Err(format!(
                        "@Redirect {} found no call to {} in {}",
                        hook_name, m.funcs[*callee as usize].name, tname
                    ));
                }
                "Redirect"
            }
            Kind::RedirectRt(rt) => {
                if redirect(&mut t.code, *rt, x.hook) == 0 {
                    return Err(format!("@Redirect {} found no matching rt call in {}", hook_name, tname));
                }
                "Redirect"
            }
            Kind::RedirectHost(h) => {
                if redirect(&mut t.code, Op::Host(*h), x.hook) == 0 {
                    return Err(format!("@Redirect {} found no matching host call in {}", hook_name, tname));
                }
                "Redirect"
            }
        };
        m.annotations.push(Annotation {
            target: Target::Func(x.target),
            name: "Mixed".into(),
            args: vec![("by".into(), Value::Str(hook_name.clone())), ("kind".into(), Value::Str(what.into()))],
        });
        log.push(format!("{}: {} by {}", tname, what, hook_name));
    }
    for x in &mixins {
        m.annotations
            .retain(|a| !(KINDS.contains(&a.name.as_str()) && a.target == Target::Func(x.hook)));
        let tname = Value::Str(m.funcs[x.target as usize].name.clone());
        if !m
            .annotations
            .iter()
            .any(|a| a.name == "Applied" && a.target == Target::Func(x.hook) && a.arg("target") == Some(&tname))
        {
            m.annotations.push(Annotation {
                target: Target::Func(x.hook),
                name: "Applied".into(),
                args: vec![("target".into(), tname)],
            });
        }
    }
    Ok(log)
}

fn redirect(code: &mut [Op], from: Op, hook: u32) -> usize {
    let mut n = 0;
    for op in code.iter_mut() {
        if *op == from {
            *op = Op::Call(hook);
            n += 1;
        }
    }
    n
}
