use crate::source::Span;
use crate::types::{TyId, Types};
use burn_runtime::RtFn;

pub type FuncId = u32;

pub use bvm::Cmp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    IAdd,
    ISub,
    IMul,
    IDiv(u32),
    IMod(u32),
    FAdd,
    FSub,
    FMul,
    FDiv,
    ICmp(Cmp),
    FCmp(Cmp),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    INeg,
    FNeg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conv {
    IntToFloat,
    FloatToInt,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: TyId,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Int(i64),
    TypeId(u32),
    LocId(u32),
    Float(f64),
    Bool(bool),
    Str(u32),
    Null,
    Local(u32),
    Global(u32),
    SetLocal(u32, Box<Expr>),
    SetGlobal(u32, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Conv(Conv, Box<Expr>),
    Call(FuncId, Vec<Expr>),
    CallIndirect(Box<Expr>, Vec<Expr>),
    CallIface(u32, Vec<Expr>),
    Rt(RtFn, Vec<Expr>),
    Spawn(FuncId, Vec<Expr>),
    FuncRef(FuncId),
    NewStruct(TyId, Vec<Expr>),
    GetField(Box<Expr>, u32),
    SetField(Box<Expr>, u32, Box<Expr>),
    NewArray(TyId, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>, u32),
    SetIndex(Box<Expr>, Box<Expr>, Box<Expr>, u32),
    ArrLen(Box<Expr>),
    BoxVal(Box<Expr>),
    Seq(Vec<Stmt>, Box<Expr>),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Expr(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    Loop { cond: Option<Expr>, body: Vec<Stmt>, step: Vec<Stmt> },
    Return(Option<Expr>),
    Break,
    Continue,
}

#[derive(Clone, Debug)]
pub struct Func {
    pub name: String,
    pub params: u32,
    pub locals: Vec<TyId>,
    pub ret: TyId,
    pub body: Vec<Stmt>,
    pub is_async: bool,
    pub span: Span,
    pub end_loc: u32,
}

#[derive(Clone, Debug)]
pub struct Global {
    pub name: String,
    pub ty: TyId,
    pub module: String,
}

#[derive(Clone, Debug)]
pub struct IfaceSlot {
    pub name: String,
    pub argc: u32,
    pub impls: Vec<(TyId, FuncId)>,
}

pub struct Program {
    pub types: Types,
    pub funcs: Vec<Func>,
    pub globals: Vec<Global>,
    pub entry: FuncId,
    pub strings: Vec<String>,
    pub locs: Vec<String>,
    pub slots: Vec<IfaceSlot>,
    pub inits: Vec<(String, FuncId)>,
    pub main: Option<FuncId>,
}

impl Program {
    pub fn meta(&self) -> burn_runtime::meta::Meta {
        burn_runtime::meta::Meta {
            types: self.types.descs(),
            locs: self.locs.clone(),
        }
    }
}

impl Expr {
    pub fn new(kind: ExprKind, ty: TyId) -> Expr {
        Expr { kind, ty }
    }

    pub fn int(v: i64) -> Expr {
        Expr {
            kind: ExprKind::Int(v),
            ty: crate::types::T_INT,
        }
    }

    pub fn has_side_effects(&self) -> bool {
        !matches!(
            self.kind,
            ExprKind::Int(_)
                | ExprKind::TypeId(_)
                | ExprKind::LocId(_)
                | ExprKind::Float(_)
                | ExprKind::Bool(_)
                | ExprKind::Str(_)
                | ExprKind::Null
                | ExprKind::Local(_)
                | ExprKind::Global(_)
                | ExprKind::FuncRef(_)
        )
    }
}
