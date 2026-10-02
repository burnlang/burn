use crate::op::Op;
use burn_runtime::meta::{builtin_descs, Desc, Meta};

pub const FIRST_USER_TYPE: u32 = 12;

#[derive(Clone, Debug, PartialEq)]
pub struct Sig {
    pub params: Vec<u32>,
    pub ret: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: u32,
    pub locals: u32,
    pub names: Vec<String>,
    pub code: Vec<Op>,
    pub external: bool,
    pub sig: Option<Sig>,
}

impl Function {
    pub fn external(name: &str, params: u32, sig: Option<Sig>) -> Function {
        Function {
            name: name.to_string(),
            params,
            locals: params,
            external: true,
            sig,
            ..Function::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Module,
    Func(u32),
    Type(u32),
    Global(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub target: Target,
    pub name: String,
    pub args: Vec<(String, Value)>,
}

impl Annotation {
    pub fn arg(&self, key: &str) -> Option<&Value> {
        self.args.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Import {
    pub name: String,
    pub argc: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub name: String,
    pub argc: u32,
    pub entries: Vec<(u32, u32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub name: String,
    pub types: Vec<Desc>,
    pub locs: Vec<String>,
    pub strings: Vec<String>,
    pub globals: Vec<String>,
    pub imports: Vec<Import>,
    pub funcs: Vec<Function>,
    pub tables: Vec<Table>,
    pub entry: Option<u32>,
    pub annotations: Vec<Annotation>,
}

impl Default for Module {
    fn default() -> Self {
        Module::new()
    }
}

impl Module {
    pub fn new() -> Module {
        Module {
            name: String::new(),
            types: builtin_descs(),
            locs: Vec::new(),
            strings: Vec::new(),
            globals: Vec::new(),
            imports: Vec::new(),
            funcs: Vec::new(),
            tables: Vec::new(),
            entry: None,
            annotations: Vec::new(),
        }
    }

    pub fn meta(&self) -> Meta {
        Meta {
            types: self.types.clone(),
            locs: self.locs.clone(),
            info: Vec::new(),
        }
    }

    pub fn func(&self, name: &str) -> Option<u32> {
        self.funcs.iter().position(|f| f.name == name).map(|i| i as u32)
    }

    pub fn annotations_of(&self, target: Target) -> impl Iterator<Item = &Annotation> {
        self.annotations.iter().filter(move |a| a.target == target)
    }

    pub fn externals(&self) -> impl Iterator<Item = &Function> {
        self.funcs.iter().filter(|f| f.external)
    }

    pub fn find_type(&self, d: &Desc) -> Option<u32> {
        self.types.iter().position(|t| t == d).map(|i| i as u32)
    }

    pub fn intern_type(&mut self, d: Desc) -> u32 {
        match self.find_type(&d) {
            Some(i) => i,
            None => {
                self.types.push(d);
                self.types.len() as u32 - 1
            }
        }
    }

    pub fn intern_string(&mut self, s: &str) -> u32 {
        match self.strings.iter().position(|x| x == s) {
            Some(i) => i as u32,
            None => {
                self.strings.push(s.to_string());
                self.strings.len() as u32 - 1
            }
        }
    }

    pub fn code_size(&self) -> usize {
        self.funcs.iter().map(|f| f.code.len()).sum()
    }
}
