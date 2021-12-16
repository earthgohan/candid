use crate::parser::typing::TypeEnv;
use crate::pretty::*;
use crate::types::{Field, Function, Label, Type};
use pretty::RcDoc;

// The definition of tuple is language specific.
pub(crate) fn is_tuple(fs: &[Field]) -> bool {
    if fs.is_empty() {
        return false;
    }
    for (i, field) in fs.iter().enumerate() {
        if field.id.get_id() != (i as u32) {
            return false;
        }
    }
    true
}
static KEYWORDS: [&str; 51] = [
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
];
pub fn ident(id: &str) -> RcDoc {
    if id.is_empty()
        || id.starts_with(|c: char| !c.is_ascii_alphabetic() && c != '_')
        || id.chars().any(|c| !c.is_ascii_alphanumeric() && c != '_')
    {
        RcDoc::as_string(format!("_{}_", crate::idl_hash(id)))
    } else if ["crate", "self", "super", "Self"].contains(&id) {
        str(id).append("_")
    } else if KEYWORDS.contains(&id) {
        str("r#").append(id)
    } else {
        str(id)
    }
}

fn pp_ty(ty: &Type) -> RcDoc {
    use Type::*;
    match *ty {
        Null => str("()"),
        Bool => str("bool"),
        Nat => str("candid::Nat"),
        Int => str("candid::Int"),
        Nat8 => str("u8"),
        Nat16 => str("u16"),
        Nat32 => str("u32"),
        Nat64 => str("u64"),
        Int8 => str("i8"),
        Int16 => str("i16"),
        Int32 => str("i32"),
        Int64 => str("i64"),
        Float32 => str("f32"),
        Float64 => str("f64"),
        Text => str("String"),
        Reserved => str("candid::Reserved"),
        Empty => str("candid::Empty"),
        Var(ref s) => ident(s),
        Principal => str("candid::Principal"),
        Opt(ref t) => str("Option").append(enclose("<", pp_ty(t), ">")),
        Vec(ref t) => str("Vec").append(enclose("<", pp_ty(t), ">")),
        Record(ref fs) => pp_record_fields(fs),
        Variant(_) => unreachable!(),
        Func(_) => str("candid::Func"),
        Service(_) => str("candid::Service"),
        Class(_, _) => unreachable!(),
        Knot(_) | Unknown => unreachable!(),
    }
}

fn pp_label(id: &Label) -> RcDoc {
    match id {
        Label::Named(str) => ident(str),
        Label::Id(n) | Label::Unnamed(n) => str("_").append(RcDoc::as_string(n)).append("_"),
    }
}

fn pp_record_field(field: &Field) -> RcDoc {
    pp_label(&field.id)
        .append(kwd(":"))
        .append(pp_ty(&field.ty))
}

fn pp_record_fields(fs: &[Field]) -> RcDoc {
    if is_tuple(fs) {
        let tuple = RcDoc::concat(fs.iter().map(|f| pp_ty(&f.ty).append(",")));
        enclose("(", tuple, ")")
    } else {
        let fields = concat(fs.iter().map(pp_record_field), ",");
        enclose_space("{", fields, "}")
    }
}

fn pp_variant_field(field: &Field) -> RcDoc {
    match &field.ty {
        Type::Null => pp_label(&field.id),
        Type::Record(fs) => pp_label(&field.id).append(pp_record_fields(fs)),
        _ => pp_label(&field.id).append(enclose("(", pp_ty(&field.ty), ")")),
    }
}

fn pp_variant_fields(fs: &[Field]) -> RcDoc {
    let fields = concat(fs.iter().map(pp_variant_field), ",");
    enclose_space("{", fields, "}")
}

fn pp_defs(env: &TypeEnv) -> RcDoc {
    let derive = "#[derive(CandidType, Deserialize)]";
    lines(env.0.iter().map(|(id, ty)| {
        let id = ident(id).append(" ");
        match ty {
            Type::Record(fs) => str(derive)
                .append(RcDoc::line())
                .append("struct ")
                .append(id)
                .append(pp_record_fields(fs))
                .append(RcDoc::hardline()),
            Type::Variant(fs) => str(derive)
                .append(RcDoc::line())
                .append("enum ")
                .append(id)
                .append(pp_variant_fields(fs))
                .append(RcDoc::hardline()),
            _ => kwd("type").append(id).append("= ").append(pp_ty(ty)),
        }
    }))
}

fn pp_function<'a>(id: &'a str, func: &'a Function) -> RcDoc<'a> {
    let id = ident(id);
    let args = concat(
        func.args
            .iter()
            .enumerate()
            .map(|(i, ty)| RcDoc::as_string(format!("arg{}: ", i)).append(pp_ty(ty))),
        ",",
    );
    let rets = concat(func.rets.iter().map(pp_ty), ",");
    let sig = kwd("pub fn")
        .append(id)
        .append(enclose("(", args, ")"))
        .append(kwd(" ->"))
        .append(enclose("(", rets, ")"));
    sig.append(";")
}

fn pp_actor<'a>(env: &'a TypeEnv, actor: &'a Type) -> RcDoc<'a> {
    // TODO trace to service before we figure out what canister means in Rust
    let serv = env.as_service(actor).unwrap();
    let body = RcDoc::intersperse(
        serv.iter().map(|(id, func)| {
            let func = env.as_func(func).unwrap();
            pp_function(id, func)
        }),
        RcDoc::hardline(),
    );
    kwd("pub trait SERVICE").append(enclose_space("{", body, "}"))
}

pub fn compile(env: &TypeEnv, actor: &Option<Type>) -> String {
    let header = r#"// This is an experimental feature to generate Rust binding from Candid.
// You may want to manually adjust some of the types.
"#;
    let (env, actor) = nominalize_all(env, actor);
    let doc = match &actor {
        None => pp_defs(&env),
        Some(actor) => {
            let defs = pp_defs(&env);
            let actor = pp_actor(&env, actor);
            defs.append(actor)
        }
    };
    let doc = RcDoc::text(header).append(RcDoc::line()).append(doc);
    doc.pretty(LINE_WIDTH).to_string()
}

pub enum TypePath {
    Id(String),
    Opt,
    Vec,
    RecordField(String),
    VariantField(String),
    Func(String),
    Init,
}
fn path_to_var(path: &[TypePath]) -> String {
    let name: Vec<&str> = path
        .iter()
        .map(|node| match node {
            TypePath::Id(id) => id.as_str(),
            TypePath::RecordField(f) | TypePath::VariantField(f) => f.as_str(),
            TypePath::Opt => "inner",
            TypePath::Vec => "item",
            TypePath::Func(id) => id.as_str(),
            TypePath::Init => "init",
        })
        .collect();
    name.join("_")
}
// Convert structural typing to nominal typing to fit Rust's type system
fn nominalize(env: &mut TypeEnv, path: &mut Vec<TypePath>, t: Type) -> Type {
    match t {
        Type::Opt(ty) => {
            path.push(TypePath::Opt);
            let ty = nominalize(env, path, *ty);
            path.pop();
            Type::Opt(Box::new(ty))
        }
        Type::Vec(ty) => {
            path.push(TypePath::Opt);
            let ty = nominalize(env, path, *ty);
            path.pop();
            Type::Vec(Box::new(ty))
        }
        Type::Record(fs) => {
            if matches!(
                path.last(),
                None | Some(TypePath::VariantField(_)) | Some(TypePath::Id(_))
            ) || is_tuple(&fs)
            {
                let fs: Vec<_> = fs
                    .into_iter()
                    .map(|Field { id, ty }| {
                        path.push(TypePath::RecordField(id.to_string()));
                        let ty = nominalize(env, path, ty);
                        path.pop();
                        Field { id, ty }
                    })
                    .collect();
                Type::Record(fs)
            } else {
                let new_var = path_to_var(path);
                let ty = nominalize(
                    env,
                    &mut vec![TypePath::Id(new_var.clone())],
                    Type::Record(fs),
                );
                env.0.insert(new_var.clone(), ty);
                Type::Var(new_var)
            }
        }
        Type::Variant(fs) => match path.last() {
            None | Some(TypePath::Id(_)) => {
                let fs: Vec<_> = fs
                    .into_iter()
                    .map(|Field { id, ty }| {
                        path.push(TypePath::VariantField(id.to_string()));
                        let ty = nominalize(env, path, ty);
                        path.pop();
                        Field { id, ty }
                    })
                    .collect();
                Type::Variant(fs)
            }
            Some(_) => {
                let new_var = path_to_var(path);
                let ty = nominalize(
                    env,
                    &mut vec![TypePath::Id(new_var.clone())],
                    Type::Variant(fs),
                );
                env.0.insert(new_var.clone(), ty);
                Type::Var(new_var)
            }
        },
        Type::Func(func) => Type::Func(Function {
            modes: func.modes,
            args: func
                .args
                .into_iter()
                .enumerate()
                .map(|(i, ty)| {
                    path.push(TypePath::Func(format!("arg{}", i)));
                    let ty = nominalize(env, path, ty);
                    path.pop();
                    ty
                })
                .collect(),
            rets: func
                .rets
                .into_iter()
                .enumerate()
                .map(|(i, ty)| {
                    path.push(TypePath::Func(format!("ret{}", i)));
                    let ty = nominalize(env, path, ty);
                    path.pop();
                    ty
                })
                .collect(),
        }),
        Type::Service(serv) => Type::Service(
            serv.into_iter()
                .map(|(meth, ty)| {
                    path.push(TypePath::Id(meth.to_string()));
                    let ty = nominalize(env, path, ty);
                    path.pop();
                    (meth, ty)
                })
                .collect(),
        ),
        Type::Class(args, ty) => Type::Class(
            args.into_iter()
                .map(|ty| {
                    path.push(TypePath::Init);
                    let ty = nominalize(env, path, ty);
                    path.pop();
                    ty
                })
                .collect(),
            Box::new(nominalize(env, path, *ty)),
        ),
        _ => t,
    }
}

fn nominalize_all(env: &TypeEnv, actor: &Option<Type>) -> (TypeEnv, Option<Type>) {
    let mut res = TypeEnv(Default::default());
    for (id, ty) in env.0.iter() {
        let ty = nominalize(&mut res, &mut vec![TypePath::Id(id.clone())], ty.clone());
        res.0.insert(id.to_string(), ty);
    }
    let actor = actor
        .as_ref()
        .map(|ty| nominalize(&mut res, &mut vec![], ty.clone()));
    (res, actor)
}