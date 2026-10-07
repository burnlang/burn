use super::comment::{self, DocComment};
use crate::ast::{self, Def, ItemKind as AItem, StmtKind, StructKind, TypeExpr, Vis};

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Function,
    Struct(StructKind),
    Interface,
    Enum,
    Type,
    Alias,
    Annotation,
    Value { is_const: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MemberKind {
    Field,
    Method,
    StaticMethod,
    AbstractMethod,
    StaticValue,
    Variant,
    Init,
    Destructor,
}

#[derive(Clone, Debug, Default)]
pub struct Sig {
    pub params: Vec<(String, TypeExpr)>,
    pub ret: Option<TypeExpr>,
    pub is_async: bool,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub kind: MemberKind,
    pub name: String,
    pub sig: Option<Sig>,
    pub ty: Option<TypeExpr>,
    pub default: Option<String>,
    pub doc: Option<DocComment>,
    pub private: bool,
    pub is_const: bool,
    pub annotations: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub kind: Kind,
    pub name: String,
    pub public: bool,
    pub private: bool,
    pub sig: Option<Sig>,
    pub ctor: Option<Vec<(String, TypeExpr)>>,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub members: Vec<Member>,
    pub doc: Option<DocComment>,
    pub annotations: Vec<String>,
    pub ty: Option<TypeExpr>,
    pub default: Option<String>,
}

#[derive(Clone, Debug)]
pub struct DocModule {
    pub name: String,
    pub is_std: bool,
    pub doc: Option<DocComment>,
    pub items: Vec<Item>,
}

fn slice(src: &str, span: crate::source::Span) -> String {
    let s = (span.start as usize).min(src.len());
    let e = (span.end as usize).clamp(s, src.len());
    src[s..e].split_whitespace().collect::<Vec<_>>().join(" ")
}

fn doc_at(src: &str, offset: u32) -> Option<DocComment> {
    comment::doc_before(src, offset as usize).map(|r| comment::parse(&r))
}

fn fun_sig(f: &ast::FunDecl) -> Sig {
    Sig {
        params: f.params.iter().map(|p| (p.name.name.clone(), p.ty.clone())).collect(),
        ret: f.ret.clone(),
        is_async: f.is_async,
    }
}

fn anns(src: &str, a: &[ast::Annotation]) -> Vec<String> {
    a.iter().map(|x| slice(src, x.span)).collect()
}

pub fn build(name: &str, is_std: bool, src: &str, m: &ast::Module) -> DocModule {
    let mut items = Vec::new();
    for it in &m.items {
        let doc = doc_at(src, it.span.start);
        let public = it.vis == Vis::Pub;
        let private = it.vis == Vis::Priv;
        let base = |kind: Kind, name: &str| Item {
            kind,
            name: name.to_string(),
            public,
            private,
            sig: None,
            ctor: None,
            extends: None,
            implements: Vec::new(),
            members: Vec::new(),
            doc: doc.clone(),
            annotations: anns(src, &it.annotations),
            ty: None,
            default: None,
        };
        match &it.kind {
            AItem::Fun(f) => {
                let mut i = base(Kind::Function, &f.name.name);
                i.sig = Some(fun_sig(f));
                i.annotations = anns(src, &f.annotations);
                items.push(i);
            }
            AItem::Stmt(s) => {
                if let StmtKind::Var { name, ty, init, is_const } = &s.kind {
                    let mut i = base(Kind::Value { is_const: *is_const }, &name.name);
                    i.ty = ty.clone();
                    i.default = init.as_ref().map(|e| slice(src, e.span));
                    items.push(i);
                }
            }
            AItem::Def(d) => {
                let dname = d.name().name.clone();
                match d {
                    Def::Struct {
                        kind,
                        params,
                        extends,
                        supers,
                        fields,
                        methods,
                        statics,
                        ..
                    } => {
                        let mut i = base(Kind::Struct(*kind), &dname);
                        if *kind != StructKind::Static {
                            i.ctor = Some(params.iter().map(|p| (p.name.name.clone(), p.ty.clone())).collect());
                        }
                        i.extends = extends.as_ref().map(|e| e.0.name.clone());
                        i.implements = supers.iter().map(|s| s.0.name.clone()).collect();
                        for p in params {
                            let pd = doc.as_ref().and_then(|d| d.param(&p.name.name)).map(|t| DocComment {
                                summary: comment::first_sentence(t),
                                body: vec![comment::Block::Para(t.to_string())],
                                ..Default::default()
                            });
                            i.members.push(Member {
                                kind: MemberKind::Field,
                                name: p.name.name.clone(),
                                sig: None,
                                ty: Some(p.ty.clone()),
                                default: None,
                                doc: pd,
                                private: false,
                                is_const: false,
                                annotations: Vec::new(),
                            });
                        }
                        for f in fields {
                            i.members.push(Member {
                                kind: MemberKind::Field,
                                name: f.name.name.clone(),
                                sig: None,
                                ty: Some(f.ty.clone()),
                                default: f.default.as_ref().map(|e| slice(src, e.span)),
                                doc: doc_at(src, f.ty.span.start.min(f.name.span.start)),
                                private: f.vis == Vis::Priv,
                                is_const: false,
                                annotations: Vec::new(),
                            });
                        }
                        for sv in statics {
                            i.members.push(Member {
                                kind: MemberKind::StaticValue,
                                name: sv.name.name.clone(),
                                sig: None,
                                ty: sv.ty.clone(),
                                default: Some(slice(src, sv.init.span)),
                                doc: doc_at(src, sv.name.span.start),
                                private: sv.vis == Vis::Priv,
                                is_const: sv.is_const,
                                annotations: Vec::new(),
                            });
                        }
                        for (vis, f) in methods {
                            let kind = if f.is_abstract {
                                MemberKind::AbstractMethod
                            } else if f.is_static {
                                MemberKind::StaticMethod
                            } else if f.name.name == "init" {
                                MemberKind::Init
                            } else if f.name.name == "destroy" {
                                MemberKind::Destructor
                            } else {
                                MemberKind::Method
                            };
                            i.members.push(Member {
                                kind,
                                name: f.name.name.clone(),
                                sig: Some(fun_sig(f)),
                                ty: None,
                                default: None,
                                doc: doc_at(src, f.span.start),
                                private: *vis == Vis::Priv,
                                is_const: false,
                                annotations: anns(src, &f.annotations),
                            });
                        }
                        items.push(i);
                    }
                    Def::Interface { methods, .. } => {
                        let mut i = base(Kind::Interface, &dname);
                        for m in methods {
                            i.members.push(Member {
                                kind: MemberKind::AbstractMethod,
                                name: m.name.name.clone(),
                                sig: Some(Sig {
                                    params: m.params.iter().map(|p| (p.name.name.clone(), p.ty.clone())).collect(),
                                    ret: m.ret.clone(),
                                    is_async: m.is_async,
                                }),
                                ty: None,
                                default: None,
                                doc: doc_at(src, m.name.span.start),
                                private: false,
                                is_const: false,
                                annotations: Vec::new(),
                            });
                        }
                        items.push(i);
                    }
                    Def::Enum { variants, fields, .. } => {
                        let mut i = base(Kind::Enum, &dname);
                        for (v, fs) in variants.iter().zip(fields) {
                            i.members.push(Member {
                                kind: MemberKind::Variant,
                                name: v.name.clone(),
                                sig: fs.as_ref().map(|fs| Sig {
                                    params: fs.iter().map(|p| (p.name.name.clone(), p.ty.clone())).collect(),
                                    ret: None,
                                    is_async: false,
                                }),
                                ty: None,
                                default: None,
                                doc: doc_at(src, v.span.start),
                                private: false,
                                is_const: false,
                                annotations: Vec::new(),
                            });
                        }
                        items.push(i);
                    }
                    Def::Type { fields, .. } | Def::Annotation { fields, .. } => {
                        let kind = if matches!(d, Def::Type { .. }) { Kind::Type } else { Kind::Annotation };
                        let mut i = base(kind, &dname);
                        for f in fields {
                            i.members.push(Member {
                                kind: MemberKind::Field,
                                name: f.name.name.clone(),
                                sig: None,
                                ty: Some(f.ty.clone()),
                                default: f.default.as_ref().map(|e| slice(src, e.span)),
                                doc: doc_at(src, f.ty.span.start.min(f.name.span.start)),
                                private: f.vis == Vis::Priv,
                                is_const: false,
                                annotations: Vec::new(),
                            });
                        }
                        items.push(i);
                    }
                    Def::Alias { ty, .. } => {
                        let mut i = base(Kind::Alias, &dname);
                        i.ty = Some(ty.clone());
                        items.push(i);
                    }
                }
            }
            AItem::Import(_) => {}
        }
    }
    let doc = comment::module_doc(src).map(|r| comment::parse(&r));
    DocModule {
        name: name.to_string(),
        is_std,
        doc,
        items,
    }
}
