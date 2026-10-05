use crate::lexer::TplPart;
use crate::source::Span;

#[derive(Clone, Debug)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vis {
    Default,
    Pub,
    Priv,
}

#[derive(Clone, Debug)]
pub struct Module {
    pub items: Vec<Item>,
    pub has_destroy: bool,
}

#[derive(Clone, Debug)]
pub struct Annotation {
    pub name: Ident,
    pub args: Vec<(Option<Ident>, Expr)>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub kind: ItemKind,
    pub vis: Vis,
    pub span: Span,
    pub annotations: Vec<Annotation>,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
    Import(Vec<(String, Span)>),
    Fun(FunDecl),
    Def(Def),
    Stmt(Stmt),
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeExpr,
}

#[derive(Clone, Debug)]
pub struct FunDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub body: Block,
    pub is_async: bool,
    pub is_static: bool,
    pub span: Span,
    pub annotations: Vec<Annotation>,
    pub bodyless: bool,
    pub is_abstract: bool,
    pub tparams: Vec<Ident>,
}

#[derive(Clone, Debug)]
pub struct FunSig {
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub is_async: bool,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: Ident,
    pub ty: TypeExpr,
    pub default: Option<Expr>,
    pub vis: Vis,
    pub annotations: Vec<Annotation>,
}

pub type SuperRef = (Ident, Option<Vec<Expr>>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructKind {
    Normal,
    Abstract,
    Static,
}

#[derive(Clone, Debug)]
pub struct StaticVal {
    pub name: Ident,
    pub ty: Option<TypeExpr>,
    pub init: Expr,
    pub is_const: bool,
    pub vis: Vis,
}

#[derive(Clone, Debug)]
pub enum Def {
    Type {
        name: Ident,
        fields: Vec<Field>,
        tparams: Vec<Ident>,
    },
    Alias {
        name: Ident,
        ty: TypeExpr,
    },
    Interface {
        name: Ident,
        methods: Vec<FunSig>,
    },
    Struct {
        name: Ident,
        kind: StructKind,
        params: Vec<Param>,
        param_anns: Vec<Vec<Annotation>>,
        extends: Option<(Ident, Option<Vec<Expr>>)>,
        supers: Vec<(Ident, Option<Vec<Expr>>)>,
        colon_extra: usize,
        fields: Vec<Field>,
        methods: Vec<(Vis, FunDecl)>,
        statics: Vec<StaticVal>,
        tparams: Vec<Ident>,
    },
    Enum {
        name: Ident,
        variants: Vec<Ident>,
    },
    Annotation {
        name: Ident,
        fields: Vec<Field>,
    },
}

impl Def {
    pub fn tparams(&self) -> &[Ident] {
        match self {
            Def::Type { tparams, .. } | Def::Struct { tparams, .. } => tparams,
            _ => &[],
        }
    }

    pub fn name(&self) -> &Ident {
        match self {
            Def::Type { name, .. }
            | Def::Alias { name, .. }
            | Def::Interface { name, .. }
            | Def::Struct { name, .. }
            | Def::Enum { name, .. }
            | Def::Annotation { name, .. } => name,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TypeExprKind {
    Named(String, Vec<TypeExpr>),
    Array(Box<TypeExpr>),
    Optional(Box<TypeExpr>),
    Map(Box<TypeExpr>, Box<TypeExpr>),
    Func(Vec<TypeExpr>, Option<Box<TypeExpr>>),
}

#[derive(Clone, Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ForIter {
    Range(Expr, Expr, bool),
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Var {
        name: Ident,
        ty: Option<TypeExpr>,
        init: Option<Expr>,
        is_const: bool,
    },
    Expr(Expr),
    If {
        cond: Expr,
        then: Block,
        els: Option<Block>,
    },
    While {
        cond: Expr,
        body: Block,
    },
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Expr>,
        body: Block,
    },
    ForIn {
        var: Ident,
        index: Option<Ident>,
        iter: ForIter,
        body: Block,
    },
    Return(Option<Expr>),
    Break,
    Continue,
    Block(Block),
    Extend {
        target: Ident,
        func: Box<FunDecl>,
    },
    Destroy {
        target: Ident,
        args: Vec<Expr>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    UShr,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::UShr => ">>>",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Int(i64),
    BigInt(u64, bool),
    Float(f64),
    Str(String),
    Template(Vec<TplExpr>),
    Bool(bool),
    Null,
    Ident(String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Assign { target: Box<Expr>, op: Option<BinOp>, value: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    Field { obj: Box<Expr>, name: Ident },
    Index { obj: Box<Expr>, index: Box<Expr> },
    Array(Vec<Expr>),
    StructLit { ty: Option<Ident>, fields: Vec<(Ident, Expr)> },
    MapLit(Vec<(Expr, Expr)>),
    Is(Box<Expr>, TypeExpr),
    As(Box<Expr>, TypeExpr),
    Await(Box<Expr>),
    Lambda(Box<FunDecl>),
    NotNull(Box<Expr>),
    New { ty: Ident, targs: Vec<TypeExpr>, args: Vec<Expr> },
    SafeGet { obj: Box<Expr>, name: Ident, args: Option<Vec<Expr>> },
    Coalesce(Box<Expr>, Box<Expr>),
    SafeAs(Box<Expr>, TypeExpr),
    Match { subject: Option<Box<Expr>>, arms: Vec<MatchArm> },
}

#[derive(Clone, Debug)]
pub enum Pattern {
    Value(Expr),
    Range(Expr, Expr, bool),
    Is(TypeExpr, Option<Ident>),
}

#[derive(Clone, Debug)]
pub enum ArmBody {
    Expr(Expr),
    Block(Block),
}

#[derive(Clone, Debug)]
pub struct MatchArm {
    pub patterns: Vec<Pattern>,
    pub guard: Option<Expr>,
    pub body: ArmBody,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TplExpr {
    Lit(String),
    Expr(Expr),
}

pub fn template_from(parts: Vec<TplPart>, f: &mut dyn FnMut(Vec<crate::lexer::Token>, Span) -> Expr) -> Vec<TplExpr> {
    parts
        .into_iter()
        .map(|p| match p {
            TplPart::Lit(s) => TplExpr::Lit(s),
            TplPart::Expr(toks, span) => TplExpr::Expr(f(toks, span)),
        })
        .collect()
}
