use crate::ast::*;
use crate::sema::{is_string_type, Sema};
use std::cell::Cell;

// Emission context threaded through the emitter: the sema pass owns the
// whole-program signature tables; cur_fn_ret_string tracks whether the
// function currently being emitted returns a crust `string`, so bare
// string-literal returns coerce to String.
struct Ctx<'a> {
    sema: &'a Sema,
    cur_fn_ret_string: Cell<bool>,
}

pub fn emit(program: Program, sema: &Sema) -> String {
    let ctx = Ctx { sema, cur_fn_ret_string: Cell::new(false) };
    let mut out = String::from("#![allow(unused_parens, unused_mut)]\n\n");
    for item in program.items {
        match item {
            Item::Use(u)          => { out.push_str(&format!("use {};\n", u.path)); }
            Item::TypeAlias(t)    => emit_type_alias(&t, &mut out),
            Item::Const(c)        => emit_const(&c, &mut out, &ctx),
            Item::Extern(e)       => emit_extern_block(&e, &mut out),
            Item::Trait(t)        => emit_trait(&t, &mut out, &ctx),
            Item::Struct(s)       => emit_struct(&s, &mut out),
            Item::Enum(e)         => emit_enum(&e, &mut out),
            Item::Impl(i)         => emit_impl(&i, &mut out, &ctx),
            Item::Function(f)     => emit_function(&f, &mut out, &ctx),
        }
    }
    out
}

// ── top-level items ───────────────────────────────────────────────────────────

fn emit_extern_block(e: &ExternBlock, out: &mut String) {
    out.push_str(&format!("extern \"{}\"\n{{\n", e.abi));
    for f in &e.functions {
        let params: Vec<String> = f.params.iter()
            .map(|p| format!("{}: {}", p.name, emit_type(&p.ty)))
            .collect();
        let ret = match &f.ret {
            None => String::new(),
            Some(t) => format!(" -> {}", emit_type(t)),
        };
        out.push_str(&format!("    fn {}({}){};\n", f.name, params.join(", "), ret));
    }
    out.push_str("}\n\n");
}

fn emit_type_alias(t: &TypeAlias, out: &mut String) {
    let pub_ = if t.pub_ { "pub " } else { "" };
    out.push_str(&format!("{}type {} = {};\n\n", pub_, t.name, emit_type(&t.ty)));
}

fn emit_const(c: &ConstItem, out: &mut String, ctx: &Ctx) {
    let pub_    = if c.pub_ { "pub " } else { "" };
    let keyword = if c.is_static { "static" } else { "const" };
    out.push_str(&format!("{}{} {}: {} = {};\n\n",
        pub_, keyword, c.name, emit_type(&c.ty), emit_expr(&c.value, ctx)));
}

fn emit_derives(derives: &[String], out: &mut String) {
    if !derives.is_empty() {
        out.push_str(&format!("#[derive({})]\n", derives.join(", ")));
    }
}

fn emit_struct(s: &Struct, out: &mut String) {
    emit_derives(&s.derives, out);
    let pub_ = if s.pub_ { "pub " } else { "" };
    out.push_str(&format!("{}struct {}\n{{\n", pub_, s.name));
    for field in &s.fields {
        let fpub = if field.pub_ { "pub " } else { "" };
        out.push_str(&format!("    {}{}: {},\n", fpub, field.name, emit_type(&field.ty)));
    }
    out.push_str("}\n\n");
}

fn emit_enum(e: &Enum, out: &mut String) {
    emit_derives(&e.derives, out);
    let pub_ = if e.pub_ { "pub " } else { "" };
    out.push_str(&format!("{}enum {}\n{{\n", pub_, e.name));
    for v in &e.variants {
        if v.fields.is_empty() {
            out.push_str(&format!("    {},\n", v.name));
        } else {
            let types: Vec<String> = v.fields.iter().map(emit_type).collect();
            out.push_str(&format!("    {}({}),\n", v.name, types.join(", ")));
        }
    }
    out.push_str("}\n\n");
}

fn emit_trait(t: &Trait, out: &mut String, ctx: &Ctx) {
    let pub_ = if t.pub_ { "pub " } else { "" };
    out.push_str(&format!("{}trait {}\n{{\n", pub_, t.name));
    for m in &t.methods {
        let self_param = match m.self_param {
            None                  => String::new(),
            Some(SelfParam::Value)  => "self".to_string(),
            Some(SelfParam::Ref)    => "&self".to_string(),
            Some(SelfParam::MutRef) => "&mut self".to_string(),
        };
        let params: Vec<String> = m.params.iter()
            .map(|p| format!("{}: {}", p.name, emit_type(&p.ty))).collect();
        let all_params = match (self_param.is_empty(), params.is_empty()) {
            (true,  true)  => String::new(),
            (true,  false) => params.join(", "),
            (false, true)  => self_param,
            (false, false) => format!("{}, {}", self_param, params.join(", ")),
        };
        let ret = m.ret.as_ref().map(|t| format!(" -> {}", emit_type(t))).unwrap_or_default();
        match &m.body {
            None => out.push_str(&format!("    fn {}({}){};\n", m.name, all_params, ret)),
            Some(body) => {
                out.push_str(&format!("    fn {}({}){}\n    {{\n", m.name, all_params, ret));
                ctx.cur_fn_ret_string.set(matches!(&m.ret, Some(t) if is_string_type(t)));
                emit_block(body, out, 2, ctx);
                ctx.cur_fn_ret_string.set(false);
                out.push_str("    }\n");
            }
        }
    }
    out.push_str("}\n\n");
}

fn emit_impl(i: &ImplBlock, out: &mut String, ctx: &Ctx) {
    let header = match &i.trait_name {
        None    => format!("impl {}", i.type_name),
        Some(t) => format!("impl {} for {}", t, i.type_name),
    };
    out.push_str(&format!("{}\n{{\n", header));
    for m in &i.methods {
        // trait items share the trait's visibility — pub is not permitted there
        let pub_ = if m.pub_ && i.trait_name.is_none() { "pub " } else { "" };
        out.push_str(&format!("    {}fn {}(", pub_, m.name));
        let mut first = true;
        if let Some(sp) = m.self_param {
            let s = match sp {
                SelfParam::Value  => "self",
                SelfParam::Ref    => "&self",
                SelfParam::MutRef => "&mut self",
            };
            out.push_str(s);
            first = false;
        }
        for p in &m.params {
            if !first { out.push_str(", "); }
            out.push_str(&format!("{}: {}", p.name, emit_type(&p.ty)));
            first = false;
        }
        out.push(')');
        if let Some(ret) = &m.ret {
            out.push_str(&format!(" -> {}", emit_type(ret)));
        }
        out.push_str("\n    {\n");
        ctx.cur_fn_ret_string.set(matches!(&m.ret, Some(t) if is_string_type(t)));
        emit_block(&m.body, out, 2, ctx);
        ctx.cur_fn_ret_string.set(false);
        out.push_str("    }\n\n");
    }
    out.push_str("}\n\n");
}

fn emit_function(f: &Function, out: &mut String, ctx: &Ctx) {
    ctx.cur_fn_ret_string.set(matches!(&f.ret, Some(t) if is_string_type(t)));
    let pub_ = if f.pub_ { "pub " } else { "" };
    if f.is_async && f.name == "main" {
        out.push_str("#[tokio::main]\n");
    }
    let async_ = if f.is_async { "async " } else { "" };
    out.push_str(&format!("{}{}fn {}(", pub_, async_, f.name));
    for (i, p) in f.params.iter().enumerate() {
        if i > 0 { out.push_str(", "); }
        out.push_str(&format!("{}: {}", p.name, emit_type(&p.ty)));
    }
    out.push(')');
    if let Some(ret) = &f.ret {
        out.push_str(&format!(" -> {}", emit_type(ret)));
    }
    out.push_str("\n{\n");
    if f.name == "main" {
        out.push_str("    let args: Vec<String> = std::env::args().collect();\n");
    }
    emit_block(&f.body, out, 1, ctx);
    out.push_str("}\n\n");
    ctx.cur_fn_ret_string.set(false);
}

// ── block / statements ────────────────────────────────────────────────────────

fn emit_block(block: &Block, out: &mut String, indent: usize, ctx: &Ctx) {
    for stmt in &block.stmts {
        emit_stmt(stmt, out, indent, ctx);
    }
}

fn emit_stmt(stmt: &Stmt, out: &mut String, indent: usize, ctx: &Ctx) {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::Let(l) => {
            let is_string = matches!(&l.ty, Some(t) if emit_type(t) == "String");
            let init = match (&l.init, is_string) {
                (Some(Expr::StringLit(s, _)), true) =>
                    format!(" = String::from(\"{}\")", escape_str(s)),
                (Some(Expr::BinOp(lhs, BinOp::Add, rhs, _)), true) =>
                    format!(" = format!(\"{{}}{{}}\", {}, {})", emit_expr(lhs, ctx), emit_expr(rhs, ctx)),
                (Some(e), _) => format!(" = {}", emit_expr(e, ctx)),
                (None, _)    => String::new(),
            };
            let ty = l.ty.as_ref().map(|t| format!(": {}", emit_type(t))).unwrap_or_default();
            out.push_str(&format!("{}let mut {}{}{};\n", pad, l.name, ty, init));
        }
        Stmt::Return(r) => {
            match &r.value {
                Some(Expr::StringLit(s, _)) if ctx.cur_fn_ret_string.get() =>
                    out.push_str(&format!("{}return String::from(\"{}\");\n", pad, escape_str(s))),
                Some(v) => out.push_str(&format!("{}return {};\n", pad, emit_expr(v, ctx))),
                None    => out.push_str(&format!("{}return;\n", pad)),
            }
        }
        Stmt::Break(_)    => out.push_str(&format!("{}break;\n", pad)),
        Stmt::Continue(_) => out.push_str(&format!("{}continue;\n", pad)),
        Stmt::If(s) => emit_if(s, out, indent, ctx),
        Stmt::While(s) => {
            let cond = match &s.cond {
                WhileCond::Expr(e)      => emit_expr(e, ctx),
                WhileCond::Let(p, e)    => format!("let {} = {}", emit_pattern(p), emit_expr(e, ctx)),
            };
            out.push_str(&format!("{}while {}\n{}{{\n", pad, cond, pad));
            emit_block(&s.body, out, indent + 1, ctx);
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::For(s) => {
            out.push_str(&format!("{}{{\n", pad));
            if let Some(init) = &s.init {
                match init {
                    ForInit::Decl { name, ty, init } =>
                        out.push_str(&format!("{}    let mut {}: {} = {};\n",
                            pad, name, emit_type(ty), emit_expr(init, ctx))),
                    ForInit::Expr(e) =>
                        out.push_str(&format!("{}    {};\n", pad, emit_expr(e, ctx))),
                }
            }
            let cond = s.cond.as_ref().map(|e| emit_expr(e, ctx)).unwrap_or_else(|| "true".to_string());
            out.push_str(&format!("{}    while {}\n{}    {{\n", pad, cond, pad));
            emit_block(&s.body, out, indent + 2, ctx);
            if let Some(update) = &s.update {
                out.push_str(&format!("{}        {};\n", pad, emit_expr(update, ctx)));
            }
            out.push_str(&format!("{}    }}\n{}}}\n", pad, pad));
        }
        Stmt::Switch(s) => {
            out.push_str(&format!("{}match {}\n{}{{\n", pad, emit_expr(&s.expr, ctx), pad));
            for arm in &s.arms {
                let pat = match &arm.pattern {
                    SwitchPattern::Value(e) => emit_expr(e, ctx),
                    SwitchPattern::Default  => "_".to_string(),
                };
                out.push_str(&format!("{}    {} =>\n{}    {{\n", pad, pat, pad));
                emit_block(&arm.body, out, indent + 2, ctx);
                out.push_str(&format!("{}    }}\n", pad));
            }
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::Match(s) => {
            out.push_str(&format!("{}match {}\n{}{{\n", pad, emit_expr(&s.expr, ctx), pad));
            for arm in &s.arms {
                let pats: Vec<String> = arm.patterns.iter().map(emit_pattern).collect();
                out.push_str(&format!("{}    {} =>\n{}    {{\n", pad, pats.join(" | "), pad));
                emit_block(&arm.body, out, indent + 2, ctx);
                out.push_str(&format!("{}    }}\n", pad));
            }
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::ForIn(s) => {
            // iterating a variable must not consume it — crust for-in has value
            // semantics, the collection stays usable afterwards. An explicit
            // .iter() call stays borrowed: for-in only reads, and external
            // iterator types (e.g. an MQTT connection) may not be Clone.
            let iter = match &s.iter {
                Expr::Ident(..) | Expr::FieldAccess(..) =>
                    format!("{}.clone().into_iter()", emit_expr(&s.iter, ctx)),
                Expr::MethodCall(obj, m, margs, _) if m == "iter" && margs.is_empty() =>
                    format!("{}.iter()", emit_expr(obj, ctx)),
                _ => emit_expr(&s.iter, ctx),
            };
            out.push_str(&format!("{}for {} in {}\n{}{{\n",
                pad, s.var, iter, pad));
            emit_block(&s.body, out, indent + 1, ctx);
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::Unsafe(block) => {
            out.push_str(&format!("{}unsafe\n{}{{\n", pad, pad));
            emit_block(block, out, indent + 1, ctx);
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::Expr(e) => {
            out.push_str(&format!("{}{};\n", pad, emit_expr(e, ctx)));
        }
    }
}

fn emit_if(s: &IfStmt, out: &mut String, indent: usize, ctx: &Ctx) {
    let pad = "    ".repeat(indent);
    let cond = match &s.cond {
        IfCond::Expr(e)   => emit_expr(e, ctx),
        IfCond::Let(p, e) => format!("let {} = {}", emit_pattern(p), emit_expr(e, ctx)),
    };
    out.push_str(&format!("{}if {}\n{}{{\n", pad, cond, pad));
    emit_block(&s.then_block, out, indent + 1, ctx);
    out.push_str(&format!("{}}}\n", pad));
    match &s.else_clause {
        Some(ElseClause::Block(b)) => {
            out.push_str(&format!("{}else\n{}{{\n", pad, pad));
            emit_block(b, out, indent + 1, ctx);
            out.push_str(&format!("{}}}\n", pad));
        }
        Some(ElseClause::If(elif)) => {
            // emit "else if ..." without a second leading pad
            let cond = match &elif.cond {
                IfCond::Expr(e)   => emit_expr(e, ctx),
                IfCond::Let(p, e) => format!("let {} = {}", emit_pattern(p), emit_expr(e, ctx)),
            };
            out.push_str(&format!("{}else if {}\n{}{{\n", pad, cond, pad));
            emit_block(&elif.then_block, out, indent + 1, ctx);
            out.push_str(&format!("{}}}\n", pad));
            // recurse for further else / else-if
            match &elif.else_clause {
                Some(ElseClause::Block(b)) => {
                    out.push_str(&format!("{}else\n{}{{\n", pad, pad));
                    emit_block(b, out, indent + 1, ctx);
                    out.push_str(&format!("{}}}\n", pad));
                }
                Some(ElseClause::If(next)) => {
                    let mut cur = next.as_ref();
                    loop {
                        let c = match &cur.cond {
                            IfCond::Expr(e)   => emit_expr(e, ctx),
                            IfCond::Let(p, e) => format!("let {} = {}", emit_pattern(p), emit_expr(e, ctx)),
                        };
                        out.push_str(&format!("{}else if {}\n{}{{\n", pad, c, pad));
                        emit_block(&cur.then_block, out, indent + 1, ctx);
                        out.push_str(&format!("{}}}\n", pad));
                        match &cur.else_clause {
                            Some(ElseClause::Block(b)) => {
                                out.push_str(&format!("{}else\n{}{{\n", pad, pad));
                                emit_block(b, out, indent + 1, ctx);
                                out.push_str(&format!("{}}}\n", pad));
                                break;
                            }
                            Some(ElseClause::If(next2)) => { cur = next2.as_ref(); }
                            None => break,
                        }
                    }
                }
                None => {}
            }
        }
        None => {}
    }
}

fn emit_pattern(p: &MatchPattern) -> String {
    match p {
        MatchPattern::Wildcard        => "_".to_string(),
        MatchPattern::IntLit(n)       => n.to_string(),
        MatchPattern::BoolLit(b)      => b.to_string(),
        MatchPattern::StringLit(s)    => format!("\"{}\"", escape_str(s)),
        MatchPattern::Ident(s)        => s.clone(),
        MatchPattern::Tuple(pats)     => format!("({})", pats.iter().map(emit_pattern).collect::<Vec<_>>().join(", ")),
        MatchPattern::Path(segs, bindings) => {
            let path = segs.join("::");
            if bindings.is_empty() { path } else { format!("{}({})", path, bindings.join(", ")) }
        }
    }
}

// ── expressions ───────────────────────────────────────────────────────────────

fn emit_expr(expr: &Expr, ctx: &Ctx) -> String {
    match expr {
        Expr::IntLit(n, _)    => n.to_string(),
        Expr::FloatLit(f, _)  => { let s = format!("{}", f); if s.contains('.') { s } else { format!("{}.0", s) } }
        Expr::BoolLit(b, _)   => b.to_string(),
        Expr::StringLit(s, _) => format!("\"{}\"", escape_str(s)),
        Expr::SelfExpr(_)     => "self".to_string(),
        Expr::Ident(name, _)  => {
            if name == "NULL" { "std::ptr::null_mut()".to_string() }
            else { name.clone() }
        }

        Expr::Tuple(elems, _) => format!("({})", elems.iter().map(|e| emit_expr(e, ctx)).collect::<Vec<_>>().join(", ")),
        Expr::Array(elems, _) => format!("[{}]", elems.iter().map(|e| emit_expr(e, ctx)).collect::<Vec<_>>().join(", ")),
        Expr::Index(obj, idx, _) => format!("{}[{} as usize]", emit_expr(obj, ctx), emit_expr(idx, ctx)),

        Expr::Assign(lhs, rhs, _) => format!("{} = {}", emit_expr(lhs, ctx), emit_expr(rhs, ctx)),
        Expr::CompoundAssign(lhs, op, rhs, _) => format!("{} {}= {}", emit_expr(lhs, ctx), emit_binop(op), emit_expr(rhs, ctx)),

        Expr::Unary(op, inner, _) => match op {
            UnaryOp::Neg    => format!("(-{})",  emit_expr(inner, ctx)),
            UnaryOp::Not    => format!("(!{})",  emit_expr(inner, ctx)),
            UnaryOp::Deref  => format!("(*{})",  emit_expr(inner, ctx)),
            UnaryOp::BitNot => format!("(!{})",  emit_expr(inner, ctx)),
        },

        Expr::Ref(is_mut, inner, _) => {
            if *is_mut { format!("(&mut {})", emit_expr(inner, ctx)) }
            else       { format!("(&{})",     emit_expr(inner, ctx)) }
        }

        Expr::BinOp(lhs, op, rhs, _) => {
            let lhs_is_str = matches!(lhs.as_ref(), Expr::StringLit(..));
            let rhs_is_str = matches!(rhs.as_ref(), Expr::StringLit(..));
            if matches!(op, BinOp::Add) && (lhs_is_str || rhs_is_str) {
                format!("format!(\"{{}}{{}}\", {}, {})", emit_expr(lhs, ctx), emit_expr(rhs, ctx))
            } else {
                format!("({} {} {})", emit_expr(lhs, ctx), emit_binop(op), emit_expr(rhs, ctx))
            }
        }

        Expr::FieldAccess(obj, field, _)  => format!("{}.{}", emit_expr(obj, ctx), field),
        Expr::MethodCall(obj, name, args, _) => {
            // Methods where every argument should be passed by reference
            const REF_ARGS: &[&str] = &[
                "contains", "starts_with", "ends_with", "binary_search",
                "get", "get_mut", "contains_key", "entry",
            ];
            // Vec position methods: first arg cast to usize.
            // HashMap insert/remove use string keys (not cast) or explicit &key.
            const USIZE_IDX: &[&str] = &["remove", "insert", "swap", "rotate_left", "rotate_right", "drain"];

            // closures handed to iterator adapters must let Rust infer their
            // parameter types — the item type depends on the chain, not on the
            // crust-declared element type
            const ADAPTER_FNS: &[&str] = &[
                "map", "filter", "for_each", "find", "position", "any", "all",
                "take_while", "skip_while", "flat_map", "fold", "retain", "sort_by_key",
            ];
            let args: Vec<String> = args.iter().enumerate().map(|(i, a)| {
                if REF_ARGS.contains(&name.as_str()) {
                    // add & unless arg is already a reference or a string literal
                    match a {
                        Expr::Ref(..) | Expr::StringLit(..) => emit_expr(a, ctx),
                        _ => format!("&{}", emit_expr(a, ctx)),
                    }
                } else if USIZE_IDX.contains(&name.as_str()) && i == 0 {
                    // Vec position methods take usize. A string key means HashMap:
                    // insert stores an owned String, remove borrows the literal.
                    match a {
                        Expr::StringLit(s, _) if name == "insert" =>
                            format!("String::from(\"{}\")", escape_str(s)),
                        Expr::StringLit(..) => emit_expr(a, ctx),
                        _ => format!("{} as usize", emit_expr(a, ctx)),
                    }
                } else if matches!(name.as_str(), "insert" | "push" | "push_front" | "push_back") {
                    // container element/value positions hold owned Strings
                    match a {
                        Expr::StringLit(s, _) => format!("String::from(\"{}\")", escape_str(s)),
                        _ => emit_expr(a, ctx),
                    }
                } else if ADAPTER_FNS.contains(&name.as_str()) {
                    match a {
                        Expr::Closure(params, body, _) => {
                            let ps: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                            let mut body_str = String::new();
                            emit_block(body, &mut body_str, 0, ctx);
                            format!("|{}| {{ {} }}", ps.join(", "), body_str.trim())
                        }
                        _ => emit_expr(a, ctx),
                    }
                } else {
                    emit_expr(a, ctx)
                }
            }).collect();
            if name == "collect" && args.is_empty() {
                // splitting a string yields borrowed slices; crust strings are
                // owned, so collecting them converts each piece
                if let Expr::MethodCall(_, m, _, _) = obj.as_ref()
                    && matches!(m.as_str(), "split" | "splitn" | "rsplit" | "split_whitespace" | "lines")
                {
                    return format!("{}.map(|s| s.to_string()).collect()", emit_expr(obj, ctx));
                }
            }
            if name == "join" && args.is_empty() {
                // bare join() is a thread join; unwrap the panic-propagation Result.
                // (Vec/slice join always takes a separator, so no collision.)
                return format!("{}.join().unwrap()", emit_expr(obj, ctx));
            }
            if name == "iter" && args.is_empty() {
                // crust iteration is value-semantics: iterate owned items so
                // adapters, predicates, and collect all see T rather than &T.
                // The clone is the correctness-first choice; the sema pass can
                // elide it later.
                return format!("{}.clone().into_iter()", emit_expr(obj, ctx));
            }
            if (name == "len" || name == "count") && args.is_empty() {
                return format!("({}.{}() as i64)", emit_expr(obj, ctx), name);
            }
            if name == "parse" && args.is_empty() {
                format!("{}.parse::<i64>().unwrap()", emit_expr(obj, ctx))
            } else if name == "unwrap_or" {
                // avoid the bare-parse unwrap: x.parse().unwrap_or(d) → x.parse::<i64>().unwrap_or(d)
                match obj.as_ref() {
                    Expr::MethodCall(inner, m, margs, _) if m == "parse" && margs.is_empty() =>
                        format!("{}.parse::<i64>().unwrap_or({})", emit_expr(inner, ctx), args.join(", ")),
                    _ => format!("{}.unwrap_or({})", emit_expr(obj, ctx), args.join(", ")),
                }
            } else {
                format!("{}.{}({})", emit_expr(obj, ctx), name, args.join(", "))
            }
        }

        Expr::StructLit(name, fields, _) => {
            let string_fields = ctx.sema.struct_string_fields.get(name);
            let pairs: Vec<String> = fields.iter().map(|(f, v)| {
                match v {
                    Expr::StringLit(s, _) if string_fields.is_some_and(|sf| sf.contains(f)) =>
                        format!("{}: String::from(\"{}\")", f, escape_str(s)),
                    _ => format!("{}: {}", f, emit_expr(v, ctx)),
                }
            }).collect();
            format!("{} {{ {} }}", name, pairs.join(", "))
        }

        Expr::Path(segs, args, is_call, _) => {
            let path = match segs[0].as_str() {
                "HashMap" | "HashSet" => format!("std::collections::{}", segs.join("::")),
                _ => segs.join("::"),
            };
            if *is_call || !args.is_empty() {
                let args: Vec<_> = args.iter().map(|a| emit_expr(a, ctx)).collect();
                format!("{}({})", path, args.join(", "))
            } else {
                path
            }
        }

        Expr::Call(name, args, _) => {
            // language builtins
            match name.as_str() {
                "spawn" if args.len() == 1 => {
                    // closures must move their captures into the new thread
                    let arg = match &args[0] {
                        c @ Expr::Closure(..) => format!("move {}", emit_expr(c, ctx)),
                        other => emit_expr(other, ctx),
                    };
                    return format!("std::thread::spawn({})", arg);
                }
                "sleep_ms" if args.len() == 1 =>
                    return format!("std::thread::sleep(std::time::Duration::from_millis(({}) as u64))",
                        emit_expr(&args[0], ctx)),
                "box" | "rc" | "arc" if args.len() == 1 => {
                    let ctor = match name.as_str() {
                        "box" => "Box::new",
                        "rc"  => "std::rc::Rc::new",
                        _     => "std::sync::Arc::new",
                    };
                    // string literals inside a pointer constructor become owned Strings —
                    // crust's `string` is always String, never &str
                    let arg = match &args[0] {
                        Expr::StringLit(s, _) => format!("String::from(\"{}\")", escape_str(s)),
                        other => emit_expr(other, ctx),
                    };
                    return format!("{}({})", ctor, arg);
                }
                _ => {}
            }
            // coerce string-literal args to String where the callee's crust
            // signature declares the parameter as `string`
            let string_params = ctx.sema.fn_string_params.get(name).map(|v| v.as_slice()).unwrap_or(&[]);
            let ref_params = ctx.sema.fn_ref_params.get(name).map(|v| v.as_slice()).unwrap_or(&[]);
            let args: Vec<_> = args.iter().enumerate().map(|(i, a)| {
                match a {
                    Expr::StringLit(s, _) if string_params.get(i).copied().unwrap_or(false) =>
                        format!("String::from(\"{}\")", escape_str(s)),
                    Expr::Ref(..) => emit_expr(a, ctx),
                    _ if ref_params.get(i).copied().unwrap_or(false) =>
                        format!("&{}", emit_expr(a, ctx)),
                    _ => emit_expr(a, ctx),
                }
            }).collect();
            if name == "flush" && args.is_empty() {
                return String::from("{ use std::io::Write; std::io::stdout().flush().ok(); }");
            }
            const MACROS: &[&str] = &[
                "println", "print", "eprintln", "eprint",
                "format", "panic", "assert", "assert_eq", "assert_ne",
                "vec", "todo", "unreachable", "dbg",
            ];
            if MACROS.contains(&name.as_str()) {
                format!("{}!({})", name, args.join(", "))
            } else {
                format!("{}({})", name, args.join(", "))
            }
        }

        Expr::Closure(params, body, _) => {
            let ps: Vec<String> = params.iter().map(|p| {
                match &p.ty {
                    Some(ty) => format!("{}: {}", p.name, emit_type(ty)),
                    None     => p.name.clone(),
                }
            }).collect();
            let mut body_str = String::new();
            emit_block(body, &mut body_str, 0, ctx);
            format!("|{}| {{ {} }}", ps.join(", "), body_str.trim())
        }

        Expr::Range(start, end, inclusive, _) => {
            if *inclusive {
                format!("{}..={}", emit_expr(start, ctx), emit_expr(end, ctx))
            } else {
                format!("{}..{}", emit_expr(start, ctx), emit_expr(end, ctx))
            }
        }

        Expr::ExternCall(name, args, _) => {
            let args: Vec<String> = args.iter().map(|a| match a {
                Expr::StringLit(s, _) => {
                    format!("b\"{}\\0\".as_ptr() as *const i8 as *mut i8", escape_str(s))
                }
                _ => emit_expr(a, ctx),
            }).collect();
            format!("unsafe {{ {}({}) }}", name, args.join(", "))
        }
        Expr::Question(inner, _) => format!("{}?", emit_expr(inner, ctx)),
        Expr::Await(inner, _)    => format!("({}).await", emit_expr(inner, ctx)),
        Expr::Cast(inner, ty, _) => format!("({} as {})", emit_expr(inner, ctx), emit_type(ty)),
    }
}

fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"'  => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            c if (c as u32) < 0x20 || c == '\x7f' => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn emit_binop(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",  BinOp::Sub => "-",
        BinOp::Mul => "*",  BinOp::Div => "/",  BinOp::Mod => "%",
        BinOp::Eq  => "==", BinOp::Ne  => "!=",
        BinOp::Lt  => "<",  BinOp::Le  => "<=",
        BinOp::Gt  => ">",  BinOp::Ge  => ">=",
        BinOp::And => "&&", BinOp::Or  => "||",
        BinOp::BitAnd => "&",  BinOp::BitOr  => "|",  BinOp::BitXor => "^",
        BinOp::Shl    => "<<", BinOp::Shr    => ">>",
    }
}

fn emit_type(ty: &Type) -> String {
    match ty {
        Type::Named(name) if name == "thread" => "std::thread::JoinHandle<()>".to_string(),
        Type::Named(name)        => name.clone(),
        Type::Generic(name, args) => {
            let name = match name.as_str() {
                "box"     => "Box",
                "rc"      => "std::rc::Rc",
                "arc"     => "std::sync::Arc",
                "HashMap" => "std::collections::HashMap",
                "HashSet" => "std::collections::HashSet",
                other     => other,
            };
            format!("{}<{}>", name, args.iter().map(emit_type).collect::<Vec<_>>().join(", "))
        }
        Type::Ptr(inner)         => format!("*mut {}", emit_type(inner)),
        Type::Ref(inner)         => format!("&{}", emit_type(inner)),
        Type::MutRef(inner)      => format!("&mut {}", emit_type(inner)),
        Type::Tuple(types)       => format!("({})", types.iter().map(emit_type).collect::<Vec<_>>().join(", ")),
        Type::Array(inner, Some(n)) => format!("[{}; {}]", emit_type(inner), n),
        Type::Array(inner, None)    => format!("[{}]", emit_type(inner)),
        Type::Fn(params, ret)    => format!("fn({}) -> {}", params.iter().map(emit_type).collect::<Vec<_>>().join(", "), emit_type(ret)),
        // borrow, don't consume: C callers expect to keep using the value they passed
        Type::ImplTrait(name)    => format!("&impl {}", name),
    }
}
