use crate::module::{Annotation, Function, Import, Module, Sig, Table, Target, Value};
use crate::op::{Cmp, Op};
use burn_runtime::meta::Desc;
use burn_runtime::RtFn;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Label(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FuncId(pub u32);

pub struct FuncBuilder {
    params: u32,
    locals: u32,
    names: Vec<String>,
    code: Vec<Op>,
    labels: Vec<Option<u32>>,
    fixups: Vec<(usize, Label)>,
    sig: Option<Sig>,
}

impl FuncBuilder {
    pub fn new(params: u32) -> FuncBuilder {
        FuncBuilder {
            params,
            locals: params,
            names: Vec::new(),
            code: Vec::new(),
            labels: Vec::new(),
            fixups: Vec::new(),
            sig: None,
        }
    }

    pub fn set_sig(&mut self, params: Vec<u32>, ret: u32) {
        self.sig = Some(Sig { params, ret });
    }

    pub fn with_params(names: &[&str]) -> FuncBuilder {
        let mut f = FuncBuilder::new(names.len() as u32);
        f.names = names.iter().map(|s| s.to_string()).collect();
        f
    }

    pub fn params(&self) -> u32 {
        self.params
    }

    pub fn local(&mut self, name: &str) -> u32 {
        if !self.names.is_empty() || !name.is_empty() {
            while (self.names.len() as u32) < self.locals {
                let n = format!("v{}", self.names.len());
                self.names.push(n);
            }
            self.names.push(name.to_string());
        }
        self.locals += 1;
        self.locals - 1
    }

    pub fn reserve_locals(&mut self, n: u32) {
        self.locals = self.locals.max(n);
    }

    pub fn set_names(&mut self, names: Vec<String>) {
        self.names = names;
    }

    pub fn len(&self) -> usize {
        self.code.len()
    }

    pub fn is_empty(&self) -> bool {
        self.code.is_empty()
    }

    pub fn last(&self) -> Option<Op> {
        self.code.last().copied()
    }

    pub fn emit(&mut self, op: Op) -> &mut Self {
        self.code.push(op);
        self
    }

    pub fn reachable_end(&self) -> bool {
        let here = Some(self.code.len() as u32);
        !self.code.last().is_some_and(|op| op.is_terminator()) || self.labels.contains(&here)
    }

    pub fn label(&mut self) -> Label {
        self.labels.push(None);
        Label(self.labels.len() as u32 - 1)
    }

    pub fn bind(&mut self, l: Label) -> &mut Self {
        self.labels[l.0 as usize] = Some(self.code.len() as u32);
        self
    }

    pub fn here(&mut self) -> Label {
        let l = self.label();
        self.bind(l);
        l
    }

    fn jump(&mut self, op: Op, l: Label) -> &mut Self {
        self.fixups.push((self.code.len(), l));
        self.code.push(op);
        self
    }

    pub fn jmp(&mut self, l: Label) -> &mut Self {
        self.jump(Op::Jmp(0), l)
    }

    pub fn jz(&mut self, l: Label) -> &mut Self {
        self.jump(Op::Jz(0), l)
    }

    pub fn jnz(&mut self, l: Label) -> &mut Self {
        self.jump(Op::Jnz(0), l)
    }

    pub fn jz_keep(&mut self, l: Label) -> &mut Self {
        self.jump(Op::JzKeep(0), l)
    }

    pub fn jnz_keep(&mut self, l: Label) -> &mut Self {
        self.jump(Op::JnzKeep(0), l)
    }

    pub fn int(&mut self, v: i64) -> &mut Self {
        self.emit(Op::Const(v as u64))
    }

    pub fn float(&mut self, v: f64) -> &mut Self {
        self.emit(Op::Const(v.to_bits()))
    }

    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.emit(Op::Const(v as u64))
    }

    pub fn load(&mut self, slot: u32) -> &mut Self {
        self.emit(Op::Load(slot))
    }

    pub fn store(&mut self, slot: u32) -> &mut Self {
        self.emit(Op::Store(slot))
    }

    pub fn icmp(&mut self, c: Cmp) -> &mut Self {
        self.emit(Op::ICmp(c))
    }

    pub fn call(&mut self, f: FuncId) -> &mut Self {
        self.emit(Op::Call(f.0))
    }

    pub fn rt(&mut self, f: RtFn) -> &mut Self {
        self.emit(Op::Rt(f))
    }

    pub fn ret(&mut self) -> &mut Self {
        self.emit(Op::Ret)
    }

    pub fn ret_void(&mut self) -> &mut Self {
        self.emit(Op::RetVoid)
    }

    pub fn finish(mut self, name: &str) -> Function {
        for (at, l) in std::mem::take(&mut self.fixups) {
            let t = self.labels[l.0 as usize].unwrap_or_else(|| panic!("label {} in {} was never bound", l.0, name));
            self.code[at] = self.code[at].with_jump_target(t);
        }
        if !self.names.is_empty() {
            while (self.names.len() as u32) < self.locals {
                let n = format!("v{}", self.names.len());
                self.names.push(n);
            }
        }
        Function {
            name: name.to_string(),
            params: self.params,
            locals: self.locals,
            names: self.names,
            code: self.code,
            external: false,
            sig: self.sig,
        }
    }
}

#[derive(Default)]
pub struct ModuleBuilder {
    m: Module,
    strings: HashMap<String, u32>,
    names: HashMap<String, u32>,
    defined: Vec<bool>,
}

impl ModuleBuilder {
    pub fn new() -> ModuleBuilder {
        ModuleBuilder::default()
    }

    pub fn string(&mut self, s: &str) -> u32 {
        if let Some(i) = self.strings.get(s) {
            return *i;
        }
        self.m.strings.push(s.to_string());
        let i = self.m.strings.len() as u32 - 1;
        self.strings.insert(s.to_string(), i);
        i
    }

    pub fn loc(&mut self, s: &str) -> u32 {
        self.m.locs.push(s.to_string());
        self.m.locs.len() as u32 - 1
    }

    pub fn global(&mut self, name: &str) -> u32 {
        self.m.globals.push(name.to_string());
        self.m.globals.len() as u32 - 1
    }

    pub fn import(&mut self, name: &str, argc: u32) -> u32 {
        if let Some(i) = self.m.imports.iter().position(|x| x.name == name) {
            return i as u32;
        }
        self.m.imports.push(Import { name: name.to_string(), argc });
        self.m.imports.len() as u32 - 1
    }

    pub fn ty(&mut self, d: Desc) -> u32 {
        self.m.intern_type(d)
    }

    pub fn new_type(&mut self, d: Desc) -> u32 {
        self.m.types.push(d);
        self.m.types.len() as u32 - 1
    }

    pub fn set_type(&mut self, tid: u32, d: Desc) {
        self.m.types[tid as usize] = d;
    }

    pub fn declare(&mut self, name: &str, params: u32) -> FuncId {
        if let Some(i) = self.names.get(name) {
            return FuncId(*i);
        }
        self.m.funcs.push(Function {
            name: name.to_string(),
            params,
            locals: params,
            ..Function::default()
        });
        self.defined.push(false);
        let id = self.m.funcs.len() as u32 - 1;
        self.names.insert(name.to_string(), id);
        FuncId(id)
    }

    pub fn external(&mut self, name: &str, params: u32, sig: Option<Sig>) -> FuncId {
        if let Some(i) = self.names.get(name) {
            return FuncId(*i);
        }
        self.m.funcs.push(Function::external(name, params, sig));
        self.defined.push(true);
        let id = self.m.funcs.len() as u32 - 1;
        self.names.insert(name.to_string(), id);
        FuncId(id)
    }

    pub fn set_name(&mut self, name: &str) {
        self.m.name = name.to_string();
    }

    pub fn annotate(&mut self, target: Target, name: &str, args: Vec<(String, Value)>) {
        self.m.annotations.push(Annotation {
            target,
            name: name.to_string(),
            args,
        });
    }

    pub fn lookup(&self, name: &str) -> Option<FuncId> {
        self.names.get(name).map(|i| FuncId(*i))
    }

    pub fn params_of(&self, f: FuncId) -> u32 {
        self.m.funcs[f.0 as usize].params
    }

    pub fn define(&mut self, id: FuncId, f: FuncBuilder) {
        let name = self.m.funcs[id.0 as usize].name.clone();
        self.m.funcs[id.0 as usize] = f.finish(&name);
        self.defined[id.0 as usize] = true;
    }

    pub fn function(&mut self, name: &str, f: FuncBuilder) -> FuncId {
        let id = self.declare(name, f.params());
        self.define(id, f);
        id
    }

    pub fn table(&mut self, name: &str, argc: u32) -> u32 {
        self.m.tables.push(Table {
            name: name.to_string(),
            argc,
            entries: Vec::new(),
        });
        self.m.tables.len() as u32 - 1
    }

    pub fn table_entry(&mut self, table: u32, tid: u32, f: FuncId) {
        self.m.tables[table as usize].entries.push((tid, f.0));
    }

    pub fn entry(&mut self, f: FuncId) {
        self.m.entry = Some(f.0);
    }

    pub fn undefined(&self) -> Vec<String> {
        self.defined
            .iter()
            .enumerate()
            .filter(|(_, d)| !**d)
            .map(|(i, _)| self.m.funcs[i].name.clone())
            .collect()
    }

    pub fn module(&self) -> &Module {
        &self.m
    }

    pub fn finish(self) -> Module {
        self.m
    }
}
