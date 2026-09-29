use crate::module::{Annotation, Function, Import, Module, Sig, Table, Target, FIRST_USER_TYPE};
use crate::op::{Op, NO_LOC};
use burn_runtime::meta::{builtin_descs, Desc};
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct LinkOptions {
    pub allow_unresolved: bool,
    pub skip_mixins: bool,
}

fn map_desc(d: &Desc, f: &dyn Fn(u32) -> u32) -> Desc {
    match d {
        Desc::Array(e) => Desc::Array(f(*e)),
        Desc::Map(k, v) => Desc::Map(f(*k), f(*v)),
        Desc::Optional(t) => Desc::Optional(f(*t)),
        Desc::Future(t) => Desc::Future(f(*t)),
        Desc::Record {
            name,
            fields,
            class,
            implements,
        } => Desc::Record {
            name: name.clone(),
            fields: fields.iter().map(|(n, t)| (n.clone(), f(*t))).collect(),
            class: *class,
            implements: implements.iter().map(|t| f(*t)).collect(),
        },
        other => other.clone(),
    }
}

fn merge_types(modules: &[&Module]) -> Result<(Vec<Desc>, Vec<Vec<u32>>), String> {
    let mut all: Vec<Desc> = builtin_descs();
    let mut maps: Vec<Vec<u32>> = Vec::with_capacity(modules.len());
    for m in modules {
        if m.types.len() < FIRST_USER_TYPE as usize || m.types[..FIRST_USER_TYPE as usize] != all[..FIRST_USER_TYPE as usize] {
            return Err(format!("module {} does not start with the built-in types", display(m)));
        }
        let mut map: Vec<u32> = (0..FIRST_USER_TYPE).collect();
        for _ in FIRST_USER_TYPE as usize..m.types.len() {
            map.push(all.len() as u32);
            all.push(Desc::Error);
        }
        for (i, d) in m.types.iter().enumerate().skip(FIRST_USER_TYPE as usize) {
            let bad = std::cell::Cell::new(None);
            let nd = map_desc(d, &|t| match map.get(t as usize) {
                Some(n) => *n,
                None => {
                    bad.set(Some(t));
                    0
                }
            });
            if let Some(t) = bad.get() {
                return Err(format!("type #{} of module {} refers to type #{}, which does not exist", i, display(m), t));
            }
            all[map[i] as usize] = nd;
        }
        maps.push(map);
    }
    let n = all.len();
    let mut canon: Vec<u32> = (0..n as u32).collect();
    loop {
        let mut changed = false;
        let mut seen: HashMap<String, u32> = HashMap::new();
        for i in 0..n {
            if canon[i] != i as u32 {
                continue;
            }
            let key = format!("{:?}", map_desc(&all[i], &|t| canon[t as usize]));
            match seen.get(&key) {
                Some(j) => {
                    canon[i] = *j;
                    changed = true;
                }
                None => {
                    seen.insert(key, i as u32);
                }
            }
        }
        for i in 0..n {
            let mut c = canon[i];
            while canon[c as usize] != c {
                c = canon[c as usize];
            }
            canon[i] = c;
        }
        if !changed {
            break;
        }
    }
    let mut compact = vec![u32::MAX; n];
    let mut out = Vec::new();
    for i in 0..n {
        if canon[i] == i as u32 {
            compact[i] = out.len() as u32;
            out.push(i);
        }
    }
    let fin = |t: u32| compact[canon[t as usize] as usize];
    let types: Vec<Desc> = out.iter().map(|i| map_desc(&all[*i], &fin)).collect();
    let maps = maps.into_iter().map(|m| m.into_iter().map(fin).collect()).collect();
    Ok((types, maps))
}

fn display(m: &Module) -> String {
    if m.name.is_empty() {
        "<unnamed>".into()
    } else {
        m.name.clone()
    }
}

fn qualified(m: &Module, f: &Function) -> String {
    if m.name.is_empty() {
        f.name.clone()
    } else {
        format!("{}::{}", m.name, f.name)
    }
}

pub fn link(modules: &[Module]) -> Result<Module, String> {
    link_with(modules, &LinkOptions::default())
}

pub fn link_with(modules: &[Module], opts: &LinkOptions) -> Result<Module, String> {
    if modules.is_empty() {
        return Err("nothing to link".into());
    }
    let refs: Vec<&Module> = modules.iter().collect();
    let (types, tmaps) = merge_types(&refs)?;
    let mut out = Module::new();
    out.name = modules[0].name.clone();
    out.types = types;

    let mut smaps = Vec::new();
    let mut string_ids: HashMap<String, u32> = HashMap::new();
    let mut locoff = Vec::new();
    let mut globoff = Vec::new();
    let mut imaps = Vec::new();
    for m in modules {
        smaps.push(
            m.strings
                .iter()
                .map(|s| {
                    *string_ids.entry(s.clone()).or_insert_with(|| {
                        out.strings.push(s.clone());
                        out.strings.len() as u32 - 1
                    })
                })
                .collect::<Vec<u32>>(),
        );
        locoff.push(out.locs.len() as u32);
        out.locs.extend(m.locs.iter().cloned());
        globoff.push(out.globals.len() as u32);
        out.globals.extend(m.globals.iter().cloned());
        let mut im = Vec::new();
        for i in &m.imports {
            match out.imports.iter().position(|x| x.name == i.name) {
                Some(p) if out.imports[p].argc != i.argc => {
                    return Err(format!(
                        "import {} is declared with {} arguments in one module and {} in another",
                        i.name, out.imports[p].argc, i.argc
                    ))
                }
                Some(p) => im.push(p as u32),
                None => {
                    out.imports.push(Import {
                        name: i.name.clone(),
                        argc: i.argc,
                    });
                    im.push(out.imports.len() as u32 - 1);
                }
            }
        }
        imaps.push(im);
    }

    let mut defs: Vec<(usize, usize)> = Vec::new();
    for (mi, m) in modules.iter().enumerate() {
        for (fi, f) in m.funcs.iter().enumerate() {
            if !f.external {
                defs.push((mi, fi));
            }
        }
    }
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    for (k, (mi, fi)) in defs.iter().enumerate() {
        let m = &modules[*mi];
        let f = &m.funcs[*fi];
        by_name.entry(f.name.clone()).or_default().push(k);
        if let Some(e) = export_name(m, *fi as u32).filter(|e| *e != f.name) {
            by_name.entry(e).or_default().push(k);
        }
        if !m.name.is_empty() {
            by_name.entry(qualified(m, f)).or_default().push(k);
        }
    }
    let mut fmaps: Vec<Vec<u32>> = modules.iter().map(|m| vec![u32::MAX; m.funcs.len()]).collect();
    for (k, (mi, fi)) in defs.iter().enumerate() {
        fmaps[*mi][*fi] = k as u32;
    }
    let mut unresolved: Vec<(usize, usize)> = Vec::new();
    let mut unresolved_ids: HashMap<String, u32> = HashMap::new();
    for (mi, m) in modules.iter().enumerate() {
        for (fi, f) in m.funcs.iter().enumerate() {
            if !f.external {
                continue;
            }
            let own = |k: &usize| defs[*k].0 != mi;
            let found: Vec<usize> = by_name.get(&f.name).map(|v| v.iter().copied().filter(own).collect()).unwrap_or_default();
            match found.as_slice() {
                [k] => {
                    let (dm, df) = defs[*k];
                    let d = &modules[dm].funcs[df];
                    if d.params != f.params {
                        return Err(format!(
                            "{} is declared with {} parameters in module {} but defined with {} in module {}",
                            f.name,
                            f.params,
                            display(m),
                            d.params,
                            display(&modules[dm])
                        ));
                    }
                    if let (Some(a), Some(b)) = (&f.sig, &d.sig) {
                        let ma = remap_sig(a, &tmaps[mi]);
                        let mb = remap_sig(b, &tmaps[dm]);
                        if ma != mb {
                            return Err(format!(
                                "{} has different parameter or return types in module {} and module {}",
                                f.name,
                                display(m),
                                display(&modules[dm])
                            ));
                        }
                    }
                    fmaps[mi][fi] = *k as u32;
                }
                [] if opts.allow_unresolved => {
                    let next = (defs.len() + unresolved.len()) as u32;
                    let id = *unresolved_ids.entry(f.name.clone()).or_insert_with(|| {
                        unresolved.push((mi, fi));
                        next
                    });
                    fmaps[mi][fi] = id;
                }
                [] => return Err(format!("module {} needs function {}, but no linked module defines it", display(m), f.name)),
                many => {
                    let where_: Vec<String> = many.iter().map(|k| display(&modules[defs[*k].0])).collect();
                    return Err(format!(
                        "function {} is defined in several modules ({}); refer to it as module::{}",
                        f.name,
                        where_.join(", "),
                        f.name
                    ));
                }
            }
        }
    }

    let mut tabmaps: Vec<Vec<u32>> = Vec::new();
    for (mi, m) in modules.iter().enumerate() {
        let mut tm = Vec::new();
        for t in &m.tables {
            let entries: Vec<(u32, u32)> = t.entries.iter().map(|(tid, f)| (tmaps[mi][*tid as usize], fmaps[mi][*f as usize])).collect();
            match out.tables.iter().position(|x| x.name == t.name && x.argc == t.argc) {
                Some(p) => {
                    for e in entries {
                        if let Some(prev) = out.tables[p].entries.iter().find(|x| x.0 == e.0) {
                            if prev.1 != e.1 {
                                return Err(format!("table {} has two functions for type #{}", t.name, e.0));
                            }
                        } else {
                            out.tables[p].entries.push(e);
                        }
                    }
                    tm.push(p as u32);
                }
                None => {
                    out.tables.push(Table {
                        name: t.name.clone(),
                        argc: t.argc,
                        entries,
                    });
                    tm.push(out.tables.len() as u32 - 1);
                }
            }
        }
        tabmaps.push(tm);
    }

    let remap_op = |mi: usize, op: Op| -> Op {
        let loc = |l: u32| if l == NO_LOC { l } else { l + locoff[mi] };
        let t = |x: u32| tmaps[mi][x as usize];
        let f = |x: u32| fmaps[mi][x as usize];
        let g = |x: u32| x + globoff[mi];
        match op {
            Op::Str(i) => Op::Str(smaps[mi][i as usize]),
            Op::TypeConst(x) => Op::TypeConst(t(x)),
            Op::LocConst(x) => Op::LocConst(loc(x)),
            Op::FuncRef(x) => Op::FuncRef(f(x)),
            Op::Call(x) => Op::Call(f(x)),
            Op::GLoad(x) => Op::GLoad(g(x)),
            Op::GStore(x) => Op::GStore(g(x)),
            Op::GTee(x) => Op::GTee(g(x)),
            Op::IDiv(l) => Op::IDiv(loc(l)),
            Op::IRem(l) => Op::IRem(loc(l)),
            Op::Index(l) => Op::Index(loc(l)),
            Op::SetIndex(l) => Op::SetIndex(loc(l)),
            Op::Dispatch(x, n) => Op::Dispatch(tabmaps[mi][x as usize], n),
            Op::Host(x) => Op::Host(imaps[mi][x as usize]),
            Op::Spawn(x, n, ty) => Op::Spawn(f(x), n, t(ty)),
            Op::NewRecord(x, n) => Op::NewRecord(t(x), n),
            Op::NewArray(x, n) => Op::NewArray(t(x), n),
            o => o,
        }
    };

    for (mi, fi) in defs.iter().chain(unresolved.iter()) {
        let src = &modules[*mi].funcs[*fi];
        out.funcs.push(Function {
            name: src.name.clone(),
            params: src.params,
            locals: src.locals,
            names: src.names.clone(),
            code: src.code.iter().map(|op| remap_op(*mi, *op)).collect(),
            external: src.external,
            sig: src.sig.as_ref().map(|s| remap_sig(s, &tmaps[*mi])),
        });
    }

    for (mi, m) in modules.iter().enumerate() {
        for a in &m.annotations {
            let target = match a.target {
                Target::Module => Target::Module,
                Target::Func(i) => Target::Func(fmaps[mi][i as usize]),
                Target::Type(t) => Target::Type(tmaps[mi][t as usize]),
                Target::Global(g) => Target::Global(g + globoff[mi]),
            };
            if matches!(a.target, Target::Func(i) if m.funcs[i as usize].external) {
                continue;
            }
            out.annotations.push(Annotation {
                target,
                name: a.name.clone(),
                args: a.args.clone(),
            });
        }
    }

    out.entry = modules.iter().enumerate().find_map(|(mi, m)| m.entry.map(|e| fmaps[mi][e as usize]));

    bind_exports(&mut out);
    if !opts.skip_mixins {
        crate::mixin::apply(&mut out)?;
    }
    crate::verify::verify(&out).map_err(|e| format!("the linked module is invalid: {}", e))?;
    Ok(out)
}

fn remap_sig(s: &Sig, map: &[u32]) -> Sig {
    Sig {
        params: s.params.iter().map(|t| map[*t as usize]).collect(),
        ret: map[s.ret as usize],
    }
}

pub fn export_name(m: &Module, f: u32) -> Option<String> {
    m.annotations_of(Target::Func(f)).find(|a| a.name == "Export").map(|a| {
        a.arg("name")
            .or_else(|| a.arg("value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| m.funcs[f as usize].name.clone())
    })
}

pub fn bind_exports(m: &mut Module) {
    let mut exports: HashMap<String, u32> = HashMap::new();
    for i in 0..m.funcs.len() as u32 {
        if m.funcs[i as usize].external {
            continue;
        }
        if let Some(n) = export_name(m, i) {
            exports.insert(n, i);
        }
    }
    let bound: Vec<Option<u32>> = m
        .imports
        .iter()
        .map(|imp| exports.get(&imp.name).copied().filter(|f| m.funcs[*f as usize].params == imp.argc))
        .collect();
    if bound.iter().all(|b| b.is_none()) {
        return;
    }
    let mut keep = Vec::new();
    let mut newidx = vec![u32::MAX; m.imports.len()];
    for (i, imp) in m.imports.iter().enumerate() {
        if bound[i].is_none() {
            newidx[i] = keep.len() as u32;
            keep.push(imp.clone());
        }
    }
    for f in &mut m.funcs {
        for op in &mut f.code {
            if let Op::Host(i) = *op {
                *op = match bound[i as usize] {
                    Some(target) => Op::Call(target),
                    None => Op::Host(newidx[i as usize]),
                };
            }
        }
    }
    m.imports = keep;
}

pub fn rebase(m: &Module, base_types: &[Desc], base_locs: u32) -> Result<Module, String> {
    if base_types.len() < FIRST_USER_TYPE as usize || m.types.len() < FIRST_USER_TYPE as usize {
        return Err("both type tables must start with the built-in types".into());
    }
    let mut types: Vec<Desc> = base_types.to_vec();
    let n = m.types.len();
    let mut map: Vec<u32> = (0..n as u32).collect();
    let mut pending: Vec<usize> = Vec::new();
    for i in FIRST_USER_TYPE as usize..n {
        let d = &m.types[i];
        let forward = std::cell::Cell::new(false);
        let nd = map_desc(d, &|t| {
            if (t as usize) >= i {
                forward.set(true);
            }
            map.get(t as usize).copied().unwrap_or(0)
        });
        if !forward.get() {
            if let Some(j) = types.iter().position(|x| *x == nd) {
                map[i] = j as u32;
                continue;
            }
        }
        map[i] = types.len() as u32;
        types.push(Desc::Error);
        pending.push(i);
    }
    for i in pending {
        types[map[i] as usize] = map_desc(&m.types[i], &|t| map[t as usize]);
    }
    let mut out = m.clone();
    out.types = types;
    let t = |x: u32| map[x as usize];
    let loc = |l: u32| if l == NO_LOC { l } else { l + base_locs };
    for f in &mut out.funcs {
        if let Some(s) = &f.sig {
            f.sig = Some(Sig {
                params: s.params.iter().map(|x| t(*x)).collect(),
                ret: t(s.ret),
            });
        }
        for op in &mut f.code {
            *op = match *op {
                Op::TypeConst(x) => Op::TypeConst(t(x)),
                Op::LocConst(x) => Op::LocConst(loc(x)),
                Op::IDiv(l) => Op::IDiv(loc(l)),
                Op::IRem(l) => Op::IRem(loc(l)),
                Op::Index(l) => Op::Index(loc(l)),
                Op::SetIndex(l) => Op::SetIndex(loc(l)),
                Op::Spawn(f, n, ty) => Op::Spawn(f, n, t(ty)),
                Op::NewRecord(x, n) => Op::NewRecord(t(x), n),
                Op::NewArray(x, n) => Op::NewArray(t(x), n),
                o => o,
            };
        }
    }
    for tab in &mut out.tables {
        for e in &mut tab.entries {
            e.0 = t(e.0);
        }
    }
    for a in &mut out.annotations {
        if let Target::Type(x) = a.target {
            a.target = Target::Type(t(x));
        }
    }
    Ok(out)
}
