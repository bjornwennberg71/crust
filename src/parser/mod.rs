use crate::ast::*;
use crate::lexer::{Token, TokenKind};
use std::collections::HashSet;

pub fn parse(tokens: Vec<Token>) -> Program {
    let mut p = Parser::new(tokens);
    p.parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    known_types: HashSet<String>,
    extern_fns: HashSet<String>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        let mut known_types = HashSet::new();
        for t in &[
            "Vec", "Option", "Result", "HashMap", "HashSet", "Box", "Rc", "Arc",
            "box", "rc", "arc", "thread",
            "u8", "u16", "u32", "u64", "u128", "usize",
            "i8", "i16", "i32", "i64", "i128", "isize",
            "f32", "f64",
        ] {
            known_types.insert(t.to_string());
        }
        Self { tokens, pos: 0, known_types, extern_fns: HashSet::new() }
    }

    fn peek(&self) -> &TokenKind { &self.tokens[self.pos].kind }

    fn peek2(&self) -> &TokenKind {
        let i = (self.pos + 1).min(self.tokens.len() - 1);
        &self.tokens[i].kind
    }

    fn span(&self) -> Span { self.tokens[self.pos].span }

    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() { self.pos += 1; }
        t
    }

    fn expect(&mut self, expected: &TokenKind) -> Span {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(expected) {
            self.advance().span
        } else {
            self.error(format!("expected {}, got {}", expected.describe(), self.peek().describe()));
        }
    }

    fn error(&self, message: String) -> ! {
        crate::error::raise(self.span(), message);
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(kind) {
            self.advance(); true
        } else { false }
    }

    // ── top level ────────────────────────────────────────────────────────────

    fn parse_program(&mut self) -> Program {
        let mut items = Vec::new();
        while *self.peek() != TokenKind::Eof {
            items.push(self.parse_item());
        }
        Program { items }
    }

    fn parse_item(&mut self) -> Item {
        // optional @derive(...) before struct/enum
        let derives = self.try_parse_derives();

        let pub_ = self.eat(&TokenKind::KwPub);

        if !derives.is_empty() && !matches!(self.peek(), TokenKind::KwStruct | TokenKind::KwEnum) {
            self.error("@derive is only allowed on structs and enums".to_string());
        }

        match self.peek() {
            TokenKind::KwUse      => Item::Use(self.parse_use()),
            TokenKind::KwType     => Item::TypeAlias(self.parse_type_alias(pub_)),
            TokenKind::KwConst    => Item::Const(self.parse_const(pub_, false)),
            TokenKind::KwStatic   => Item::Const(self.parse_const(pub_, true)),
            TokenKind::KwExtern   => Item::Extern(self.parse_extern()),
            TokenKind::KwTrait    => Item::Trait(self.parse_trait(pub_)),
            TokenKind::KwStruct   => Item::Struct(self.parse_struct(pub_, derives)),
            TokenKind::KwEnum     => Item::Enum(self.parse_enum(pub_, derives)),
            TokenKind::KwImpl     => {
                // `impl Shape make()` is a function returning impl Trait;
                // `impl Point {` / `impl Shape for Point {` is an impl block
                let third = &self.tokens[(self.pos + 2).min(self.tokens.len() - 1)].kind;
                if matches!(third, TokenKind::Ident(_)) {
                    Item::Function(self.parse_function(pub_, false))
                } else {
                    if pub_ {
                        self.error("'public' is not allowed on impl blocks — mark individual methods instead".to_string());
                    }
                    Item::Impl(self.parse_impl())
                }
            }
            TokenKind::KwAsync    => {
                self.advance();
                Item::Function(self.parse_function(pub_, true))
            }
            // C-style declaration: return type first, then name
            TokenKind::KwInt | TokenKind::KwFloat | TokenKind::KwBool | TokenKind::KwVoid |
            TokenKind::KwChar | TokenKind::KwString | TokenKind::Star | TokenKind::Ampersand |
            TokenKind::LBracket | TokenKind::LParen | TokenKind::Ident(_)
                => Item::Function(self.parse_function(pub_, false)),
            other => {
                let msg = format!("expected an item (a function like 'int add(int a, int b)', struct, enum, trait, impl, use, const), got {}", other.describe());
                self.error(msg)
            }
        }
    }

    fn try_parse_derives(&mut self) -> Vec<String> {
        // #derive(A, B) — canonical, preprocessor style. Accepted aliases:
        // @derive(A, B) and Rust's #[derive(A, B)].
        let bracketed = match self.peek() {
            TokenKind::At   => { self.advance(); false }
            TokenKind::Hash => {
                self.advance();
                self.eat(&TokenKind::LBracket)   // bracket present = Rust form
            }
            _ => return Vec::new(),
        };
        match self.peek().clone() {
            TokenKind::Ident(s) if s == "derive" => { self.advance(); }
            other => self.error(format!("expected 'derive' in attribute, got {}", other.describe())),
        }
        self.expect(&TokenKind::LParen);
        let mut derives = Vec::new();
        while *self.peek() != TokenKind::RParen {
            if !derives.is_empty() { self.expect(&TokenKind::Comma); }
            derives.push(self.parse_ident());
        }
        self.expect(&TokenKind::RParen);
        if bracketed { self.expect(&TokenKind::RBracket); }
        derives
    }

    fn parse_use(&mut self) -> UseItem {
        self.expect(&TokenKind::KwUse);
        // collect everything up to the semicolon as the raw path
        let mut path = String::new();
        while *self.peek() != TokenKind::Semicolon && *self.peek() != TokenKind::Eof {
            let raw = match self.peek().clone() {
                TokenKind::Ident(s)    => s,
                TokenKind::ColonColon  => "::".to_string(),
                TokenKind::LBrace      => "{".to_string(),
                TokenKind::RBrace      => "}".to_string(),
                TokenKind::Comma       => ", ".to_string(),
                TokenKind::Star        => "*".to_string(),
                other => self.error(format!("unexpected {} in use path", other.describe())),
            };
            path.push_str(&raw);
            self.advance();
        }
        self.expect(&TokenKind::Semicolon);
        UseItem { path }
    }

    fn parse_type_alias(&mut self, pub_: bool) -> TypeAlias {
        let span = self.span();
        self.expect(&TokenKind::KwType);
        let name = self.parse_ident();
        self.expect(&TokenKind::Eq);
        let ty = self.parse_type();
        self.expect(&TokenKind::Semicolon);
        TypeAlias { span, pub_, name, ty }
    }

    fn parse_const(&mut self, pub_: bool, is_static: bool) -> ConstItem {
        let span = self.span();
        if is_static { self.expect(&TokenKind::KwStatic); } else { self.expect(&TokenKind::KwConst); }
        let ty = self.parse_type();
        let name = self.parse_ident();
        self.expect(&TokenKind::Eq);
        let value = self.parse_expr();
        self.expect(&TokenKind::Semicolon);
        ConstItem { span, pub_, is_static, name, ty, value }
    }

    fn parse_extern(&mut self) -> ExternBlock {
        self.expect(&TokenKind::KwExtern);
        let abi = match self.peek().clone() {
            TokenKind::StringLit(s) => { self.advance(); s }
            other => self.error(format!("expected ABI string after extern (e.g. extern \"C\"), got {}", other.describe())),
        };
        self.expect(&TokenKind::LBrace);
        let mut functions = Vec::new();
        while *self.peek() != TokenKind::RBrace {
            let span = self.span();
            let ret = self.parse_return_type();
            let name = self.parse_ident();
            self.extern_fns.insert(name.clone());
            self.expect(&TokenKind::LParen);
            let params = self.parse_params();
            self.expect(&TokenKind::RParen);
            self.expect(&TokenKind::Semicolon);
            functions.push(ExternFn { span, name, params, ret });
        }
        self.expect(&TokenKind::RBrace);
        ExternBlock { abi, functions }
    }

    fn parse_struct(&mut self, pub_: bool, derives: Vec<String>) -> Struct {
        let span = self.span();
        self.expect(&TokenKind::KwStruct);
        let name = self.parse_ident();
        self.known_types.insert(name.clone());
        self.expect(&TokenKind::LBrace);
        let mut fields = Vec::new();
        while *self.peek() != TokenKind::RBrace {
            let fspan = self.span();
            let fpub = self.eat(&TokenKind::KwPub);
            let ty   = self.parse_type();
            let fname = self.parse_ident();
            self.expect(&TokenKind::Semicolon);
            fields.push(StructField { span: fspan, pub_: fpub, name: fname, ty });
        }
        self.expect(&TokenKind::RBrace);
        self.eat(&TokenKind::Semicolon);
        Struct { span, pub_, derives, name, fields }
    }

    fn parse_enum(&mut self, pub_: bool, derives: Vec<String>) -> Enum {
        let span = self.span();
        self.expect(&TokenKind::KwEnum);
        let name = self.parse_ident();
        self.known_types.insert(name.clone());
        self.expect(&TokenKind::LBrace);
        let mut variants = Vec::new();
        while *self.peek() != TokenKind::RBrace {
            let vspan = self.span();
            let vname = self.parse_ident();
            let fields = if *self.peek() == TokenKind::LParen {
                self.advance();
                let mut types = Vec::new();
                while *self.peek() != TokenKind::RParen {
                    if !types.is_empty() { self.expect(&TokenKind::Comma); }
                    types.push(self.parse_type());
                }
                self.expect(&TokenKind::RParen);
                types
            } else { Vec::new() };
            self.eat(&TokenKind::Comma);
            variants.push(EnumVariant { span: vspan, name: vname, fields });
        }
        self.expect(&TokenKind::RBrace);
        self.eat(&TokenKind::Semicolon);
        Enum { span, pub_, derives, name, variants }
    }

    fn parse_impl(&mut self) -> ImplBlock {
        let span = self.span();
        self.expect(&TokenKind::KwImpl);
        let first = self.parse_ident();
        let (trait_name, type_name) = if self.eat(&TokenKind::KwFor) {
            (Some(first), self.parse_ident())
        } else {
            (None, first)
        };
        self.expect(&TokenKind::LBrace);
        let mut methods = Vec::new();
        while *self.peek() != TokenKind::RBrace {
            methods.push(self.parse_method());
        }
        self.expect(&TokenKind::RBrace);
        ImplBlock { span, trait_name, type_name, methods }
    }

    fn parse_trait(&mut self, pub_: bool) -> Trait {
        let span = self.span();
        self.expect(&TokenKind::KwTrait);
        let name = self.parse_ident();
        self.known_types.insert(name.clone());
        self.expect(&TokenKind::LBrace);
        let mut methods = Vec::new();
        while *self.peek() != TokenKind::RBrace {
            methods.push(self.parse_trait_method());
        }
        self.expect(&TokenKind::RBrace);
        Trait { span, pub_, name, methods }
    }

    fn parse_trait_method(&mut self) -> TraitMethod {
        let span = self.span();
        let _pub   = self.eat(&TokenKind::KwPub);
        let is_static  = self.eat(&TokenKind::KwStatic);
        let ret = self.parse_return_type();
        let name = self.parse_ident();
        self.expect(&TokenKind::LParen);
        let (old_self_param, params) = self.parse_method_params();
        self.expect(&TokenKind::RParen);
        let is_mutable = self.eat(&TokenKind::KwMut);
        let self_param = if is_static { None }
            else if let Some(sp) = old_self_param { Some(sp) }
            else if is_mutable { Some(SelfParam::MutRef) }
            else { Some(SelfParam::Ref) };
        let body = if *self.peek() == TokenKind::Semicolon {
            self.advance(); None
        } else {
            Some(self.parse_block())
        };
        TraitMethod { span, name, self_param, params, ret, body }
    }

    fn parse_method(&mut self) -> Method {
        let span = self.span();
        let pub_       = self.eat(&TokenKind::KwPub);
        let is_static  = self.eat(&TokenKind::KwStatic);
        let ret = self.parse_return_type();
        let name = self.parse_ident();
        self.expect(&TokenKind::LParen);
        let (old_self_param, params) = self.parse_method_params();
        self.expect(&TokenKind::RParen);
        let is_mutable = self.eat(&TokenKind::KwMut);
        let body = self.parse_block();
        let self_param = if is_static {
            None
        } else if let Some(sp) = old_self_param {
            Some(sp)                    // backward compat: explicit &self / &mut self
        } else if is_mutable {
            Some(SelfParam::MutRef)     // mutable keyword after )
        } else {
            Some(SelfParam::Ref)        // default: read-only &self
        };
        Method { span, pub_, name, self_param, params, ret, body }
    }

    fn parse_method_params(&mut self) -> (Option<SelfParam>, Vec<Param>) {
        // detect self / &self / &mut self as first param
        let self_param = match self.peek() {
            TokenKind::KwSelf => { self.advance(); Some(SelfParam::Value) }
            TokenKind::Ampersand => {
                let is_mut = matches!(self.peek2(), TokenKind::KwMut);
                // peek ahead: &self or &mut self
                if matches!(self.peek2(), TokenKind::KwSelf) || is_mut {
                    self.advance(); // consume &
                    if is_mut { self.advance(); } // consume mut
                    if *self.peek() == TokenKind::KwSelf { self.advance(); }
                    Some(if is_mut { SelfParam::MutRef } else { SelfParam::Ref })
                } else { None }
            }
            _ => None,
        };

        let mut params = Vec::new();
        while *self.peek() != TokenKind::RParen {
            if self_param.is_some() || !params.is_empty() {
                self.expect(&TokenKind::Comma);
            }
            if *self.peek() == TokenKind::RParen { break; }
            let span = self.span();
            let ty   = self.parse_type();
            let pname = self.parse_ident();
            params.push(Param { span, name: pname, ty });
        }
        (self_param, params)
    }

    fn parse_function(&mut self, pub_: bool, is_async: bool) -> Function {
        let span = self.span();
        let ret = self.parse_return_type();
        let name = self.parse_ident();
        self.expect(&TokenKind::LParen);
        let params = self.parse_params();
        self.expect(&TokenKind::RParen);
        let body = self.parse_block();
        Function { span, pub_, is_async, name, params, ret, body }
    }

    // C-style: return type in front of the name; void means no return value
    fn parse_return_type(&mut self) -> Option<Type> {
        if self.eat(&TokenKind::KwVoid) { None } else { Some(self.parse_type()) }
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        while *self.peek() != TokenKind::RParen {
            if !params.is_empty() { self.expect(&TokenKind::Comma); }
            let span = self.span();
            let mut ty = self.parse_type();
            let name   = self.parse_ident();
            // C-style array suffix: int arr[] or int arr[4]
            if self.eat(&TokenKind::LBracket) {
                let n = if let TokenKind::IntLit(n) = self.peek().clone() {
                    self.advance(); Some(n as usize)
                } else { None };
                self.expect(&TokenKind::RBracket);
                ty = Type::Array(Box::new(ty), n);
            }
            params.push(Param { span, name, ty });
        }
        params
    }

    // ── statements ───────────────────────────────────────────────────────────

    fn parse_block(&mut self) -> Block {
        let start = self.span();
        self.expect(&TokenKind::LBrace);
        let mut stmts = Vec::new();
        while *self.peek() != TokenKind::RBrace {
            stmts.push(self.parse_stmt());
        }
        let end = self.span();
        self.expect(&TokenKind::RBrace);
        Block { span: Span { start: start.start, end: end.end }, stmts }
    }

    fn parse_stmt(&mut self) -> Stmt {
        match self.peek() {
            TokenKind::KwLet    => Stmt::Let(self.parse_let()),
            TokenKind::KwReturn => Stmt::Return(self.parse_return()),
            TokenKind::KwBreak  => { let span = self.span(); self.advance(); self.eat(&TokenKind::Semicolon); Stmt::Break(span) }
            TokenKind::KwContinue => { let span = self.span(); self.advance(); self.eat(&TokenKind::Semicolon); Stmt::Continue(span) }
            TokenKind::KwIf     => Stmt::If(self.parse_if()),
            TokenKind::KwWhile  => Stmt::While(self.parse_while()),
            TokenKind::KwFor    => {
                // `for (x : xs)` / `for (auto x : xs)` / `for (x in xs)` is
                // a for-in despite the parens; C-style `for (init; cond;
                // update)` is anything else parenthesized. (Ident+Colon
                // can't start a C-style init, and `::` lexes as ColonColon.)
                let at = |i: usize| &self.tokens[i.min(self.tokens.len() - 1)].kind;
                let mut i = self.pos + 2;
                if matches!(at(i), TokenKind::KwLet) { i += 1; }   // optional auto/let
                let paren_for_in = matches!(self.peek2(), TokenKind::LParen)
                    && matches!(at(i), TokenKind::Ident(_))
                    && matches!(at(i + 1), TokenKind::KwIn | TokenKind::Colon);
                if matches!(self.peek2(), TokenKind::LParen) && !paren_for_in {
                    Stmt::For(self.parse_for())
                } else {
                    Stmt::ForIn(self.parse_for_in())
                }
            }
            TokenKind::KwSwitch => Stmt::Match(self.parse_switch()),
            TokenKind::KwMatch  => Stmt::Match(self.parse_match()),
            TokenKind::KwUnsafe => { self.advance(); Stmt::Unsafe(self.parse_block()) }
            // optional explicit `mutable` / `mut` before a type-first declaration
            TokenKind::KwMut if self.peek_is_type2() || self.peek_is_tuple_type_decl2() => {
                self.advance();
                Stmt::Let(self.parse_decl(true))
            }
            _ if self.peek_is_type() || self.peek_is_tuple_type_decl() => Stmt::Let(self.parse_decl(false)),
            _ => {
                let expr = self.parse_expr();
                self.eat(&TokenKind::Semicolon);
                Stmt::Expr(expr)
            }
        }
    }

    fn parse_let(&mut self) -> LetStmt {
        let span = self.span();
        self.expect(&TokenKind::KwLet);
        let mutable = self.eat(&TokenKind::KwMut);   // const by default
        let name = self.parse_ident();
        let init = if self.eat(&TokenKind::Eq) { Some(self.parse_expr()) } else { None };
        self.expect(&TokenKind::Semicolon);
        LetStmt { span, name, ty: None, init, mutable }
    }

    fn parse_decl(&mut self, mutable: bool) -> LetStmt {
        let span = self.span();
        let mut ty = self.parse_type();
        let name = self.parse_ident();
        // C-style array suffix: int arr[4] or int arr[]
        if self.eat(&TokenKind::LBracket) {
            let n = if let TokenKind::IntLit(n) = self.peek().clone() {
                self.advance(); Some(n as usize)
            } else { None };
            self.expect(&TokenKind::RBracket);
            ty = Type::Array(Box::new(ty), n);
        }
        let init = if self.eat(&TokenKind::Eq) { Some(self.parse_expr()) } else { None };
        self.expect(&TokenKind::Semicolon);
        LetStmt { span, name, ty: Some(ty), init, mutable }
    }

    fn parse_return(&mut self) -> ReturnStmt {
        let span = self.span();
        self.expect(&TokenKind::KwReturn);
        let value = if *self.peek() != TokenKind::Semicolon { Some(self.parse_expr()) } else { None };
        self.expect(&TokenKind::Semicolon);
        ReturnStmt { span, value }
    }

    fn parse_if(&mut self) -> IfStmt {
        let span = self.span();
        self.expect(&TokenKind::KwIf);
        let cond = if *self.peek() == TokenKind::KwLet {
            self.advance();
            let pat = self.parse_match_pattern();
            self.expect(&TokenKind::Eq);
            let expr = self.parse_expr();
            IfCond::Let(pat, Box::new(expr))
        } else if *self.peek() == TokenKind::LParen && *self.peek2() == TokenKind::KwLet {
            // if(let Pattern = expr)
            self.advance(); // (
            self.advance(); // let
            let pat = self.parse_match_pattern();
            self.expect(&TokenKind::Eq);
            let expr = self.parse_expr();
            self.expect(&TokenKind::RParen);
            IfCond::Let(pat, Box::new(expr))
        } else {
            self.expect(&TokenKind::LParen);
            let e = self.parse_expr();
            self.expect(&TokenKind::RParen);
            IfCond::Expr(e)
        };
        let then_block = self.parse_block();
        let else_clause = if self.eat(&TokenKind::KwElse) {
            if *self.peek() == TokenKind::KwIf {
                Some(ElseClause::If(Box::new(self.parse_if())))
            } else {
                Some(ElseClause::Block(self.parse_block()))
            }
        } else { None };
        IfStmt { span, cond, then_block, else_clause }
    }

    fn parse_while(&mut self) -> WhileStmt {
        let span = self.span();
        self.expect(&TokenKind::KwWhile);
        let cond = if *self.peek() == TokenKind::KwLet {
            self.advance();
            let pat = self.parse_match_pattern();
            self.expect(&TokenKind::Eq);
            let expr = self.parse_expr();
            WhileCond::Let(pat, Box::new(expr))
        } else if *self.peek() == TokenKind::LParen && *self.peek2() == TokenKind::KwLet {
            // while(let Pattern = expr)
            self.advance(); // (
            self.advance(); // let
            let pat = self.parse_match_pattern();
            self.expect(&TokenKind::Eq);
            let expr = self.parse_expr();
            self.expect(&TokenKind::RParen);
            WhileCond::Let(pat, Box::new(expr))
        } else {
            self.expect(&TokenKind::LParen);
            let e = self.parse_expr();
            self.expect(&TokenKind::RParen);
            WhileCond::Expr(e)
        };
        let body = self.parse_block();
        WhileStmt { span, cond, body }
    }

    fn parse_for_in(&mut self) -> ForInStmt {
        let span = self.span();
        self.expect(&TokenKind::KwFor);
        let parens = self.eat(&TokenKind::LParen);   // for (x : xs) — canonical; bare also accepted
        self.eat(&TokenKind::KwLet);                 // for (auto x : xs) — auto is implied, explicit ok
        let var = self.parse_ident();
        match self.peek() {
            // ':' is the canonical C++ range-for spelling, 'in' the alias
            TokenKind::Colon | TokenKind::KwIn => { self.advance(); }
            other => {
                let msg = format!("expected ':' or 'in' after for-loop variable, got {}", other.describe());
                self.error(msg)
            }
        }
        let iter = self.parse_expr();
        if parens { self.expect(&TokenKind::RParen); }
        let body = self.parse_block();
        ForInStmt { span, var, iter, body }
    }

    fn parse_for(&mut self) -> ForStmt {
        let span = self.span();
        self.expect(&TokenKind::KwFor);
        self.expect(&TokenKind::LParen);
        let init = if *self.peek() == TokenKind::Semicolon {
            None
        } else if self.peek_is_type() {
            let ty   = self.parse_type();
            let name = self.parse_ident();
            self.expect(&TokenKind::Eq);
            let init = self.parse_expr();
            Some(ForInit::Decl { name, ty, init })
        } else {
            Some(ForInit::Expr(self.parse_expr()))
        };
        self.expect(&TokenKind::Semicolon);
        let cond   = if *self.peek() == TokenKind::Semicolon { None } else { Some(self.parse_expr()) };
        self.expect(&TokenKind::Semicolon);
        let update = if *self.peek() == TokenKind::RParen    { None } else { Some(self.parse_expr()) };
        self.expect(&TokenKind::RParen);
        let body = self.parse_block();
        ForStmt { span, init, cond, update, body }
    }

    fn parse_match(&mut self) -> MatchStmt {
        let span = self.span();
        self.expect(&TokenKind::KwMatch);
        let expr = self.parse_expr();
        self.expect(&TokenKind::LBrace);
        let mut arms = Vec::new();
        while *self.peek() != TokenKind::RBrace {
            let mut patterns = vec![self.parse_match_pattern()];
            while self.eat(&TokenKind::Pipe) {
                patterns.push(self.parse_match_pattern());
            }
            match self.peek() {
                TokenKind::FatArrow | TokenKind::Colon => { self.advance(); }
                other => self.error(format!("expected '=>' or ':' after match pattern, got {}", other.describe())),
            }
            let body = self.parse_block();
            arms.push(MatchArm { patterns, body });
        }
        self.expect(&TokenKind::RBrace);
        MatchStmt { span, expr, arms }
    }

    fn parse_match_pattern(&mut self) -> MatchPattern {
        match self.peek().clone() {
            TokenKind::IntLit(n)     => { self.advance(); MatchPattern::IntLit(n) }
            TokenKind::Minus => {
                // negative literal pattern: case -1:
                self.advance();
                match self.peek().clone() {
                    TokenKind::IntLit(n) => { self.advance(); MatchPattern::IntLit(-n) }
                    other => self.error(format!("expected integer after '-' in pattern, got {}", other.describe())),
                }
            }
            TokenKind::StringLit(s)  => { self.advance(); MatchPattern::StringLit(s) }
            TokenKind::KwTrue     => { self.advance(); MatchPattern::BoolLit(true) }
            TokenKind::KwFalse    => { self.advance(); MatchPattern::BoolLit(false) }
            TokenKind::KwDefault  => { self.advance(); MatchPattern::Wildcard }
            TokenKind::Ident(s) if s == "_" => { self.advance(); MatchPattern::Wildcard }
            TokenKind::Ident(s)   => {
                self.advance();
                if *self.peek() == TokenKind::ColonColon {
                    let mut segs = vec![s];
                    while self.eat(&TokenKind::ColonColon) {
                        segs.push(self.parse_ident());
                    }
                    let bindings = if *self.peek() == TokenKind::LParen {
                        self.advance();
                        let mut b = Vec::new();
                        while *self.peek() != TokenKind::RParen {
                            if !b.is_empty() { self.expect(&TokenKind::Comma); }
                            b.push(self.parse_ident());
                        }
                        self.expect(&TokenKind::RParen);
                        b
                    } else { Vec::new() };
                    MatchPattern::Path(segs, bindings)
                } else if *self.peek() == TokenKind::LParen {
                    // Some(x), None(x) style without ::
                    self.advance();
                    let mut b = Vec::new();
                    while *self.peek() != TokenKind::RParen {
                        if !b.is_empty() { self.expect(&TokenKind::Comma); }
                        b.push(self.parse_ident());
                    }
                    self.expect(&TokenKind::RParen);
                    MatchPattern::Path(vec![s], b)
                } else {
                    MatchPattern::Ident(s)
                }
            }
            TokenKind::LParen => {
                self.advance();
                let mut pats = Vec::new();
                while *self.peek() != TokenKind::RParen {
                    if !pats.is_empty() { self.expect(&TokenKind::Comma); }
                    pats.push(self.parse_match_pattern());
                }
                self.expect(&TokenKind::RParen);
                MatchPattern::Tuple(pats)
            }
            other => self.error(format!("expected match pattern, got {}", other.describe())),
        }
    }

    // switch is the canonical spelling of match: cases are full patterns,
    // including destructuring (`case Ok(s):`) and alternatives (`case 1 | 2:`).
    fn parse_switch(&mut self) -> MatchStmt {
        let span = self.span();
        self.expect(&TokenKind::KwSwitch);
        self.expect(&TokenKind::LParen);
        let expr = self.parse_expr();
        self.expect(&TokenKind::RParen);
        self.expect(&TokenKind::LBrace);
        let mut arms = Vec::new();
        while *self.peek() != TokenKind::RBrace {
            let patterns = match self.peek() {
                TokenKind::KwCase => {
                    self.advance();
                    let mut pats = vec![self.parse_match_pattern()];
                    while self.eat(&TokenKind::Pipe) {
                        pats.push(self.parse_match_pattern());
                    }
                    pats
                }
                TokenKind::KwDefault => {
                    self.advance();
                    vec![MatchPattern::Wildcard]
                }
                other => self.error(format!("expected 'case' or 'default', got {}", other.describe())),
            };
            self.expect(&TokenKind::Colon);
            let body = self.parse_block();
            arms.push(MatchArm { patterns, body });
        }
        self.expect(&TokenKind::RBrace);
        MatchStmt { span, expr, arms }
    }

    // ── expressions ──────────────────────────────────────────────────────────

    fn parse_expr(&mut self) -> Expr {
        let lhs = self.parse_assign();
        match self.peek().clone() {
            TokenKind::DotDot | TokenKind::DotDotEq => {
                let inclusive = *self.peek() == TokenKind::DotDotEq;
                let span = self.span();
                self.advance();
                let rhs = self.parse_assign();
                Expr::Range(Box::new(lhs), Box::new(rhs), inclusive, span)
            }
            _ => lhs,
        }
    }

    fn parse_assign(&mut self) -> Expr {
        let span = self.span();
        let lhs = self.parse_or();
        match self.peek() {
            TokenKind::Eq      => { self.advance(); let r = self.parse_assign(); Expr::Assign(Box::new(lhs), Box::new(r), span) }
            TokenKind::PlusEq  => { self.advance(); let r = self.parse_assign(); Expr::CompoundAssign(Box::new(lhs), BinOp::Add,    Box::new(r), span) }
            TokenKind::MinusEq => { self.advance(); let r = self.parse_assign(); Expr::CompoundAssign(Box::new(lhs), BinOp::Sub,    Box::new(r), span) }
            TokenKind::StarEq  => { self.advance(); let r = self.parse_assign(); Expr::CompoundAssign(Box::new(lhs), BinOp::Mul,    Box::new(r), span) }
            TokenKind::SlashEq => { self.advance(); let r = self.parse_assign(); Expr::CompoundAssign(Box::new(lhs), BinOp::Div,    Box::new(r), span) }
            TokenKind::PipeEq  => { self.advance(); let r = self.parse_assign(); Expr::CompoundAssign(Box::new(lhs), BinOp::BitOr,  Box::new(r), span) }
            TokenKind::AmpEq   => { self.advance(); let r = self.parse_assign(); Expr::CompoundAssign(Box::new(lhs), BinOp::BitAnd, Box::new(r), span) }
            TokenKind::CaretEq => { self.advance(); let r = self.parse_assign(); Expr::CompoundAssign(Box::new(lhs), BinOp::BitXor, Box::new(r), span) }
            TokenKind::LtLtEq  => { self.advance(); let r = self.parse_assign(); Expr::CompoundAssign(Box::new(lhs), BinOp::Shl,    Box::new(r), span) }
            TokenKind::GtGtEq  => { self.advance(); let r = self.parse_assign(); Expr::CompoundAssign(Box::new(lhs), BinOp::Shr,    Box::new(r), span) }
            _ => lhs,
        }
    }

    fn parse_or(&mut self) -> Expr {
        let mut lhs = self.parse_and();
        while *self.peek() == TokenKind::PipePipe {
            let span = self.span(); self.advance();
            let rhs = self.parse_and();
            lhs = Expr::BinOp(Box::new(lhs), BinOp::Or, Box::new(rhs), span);
        }
        lhs
    }

    fn parse_and(&mut self) -> Expr {
        let mut lhs = self.parse_bitor();
        while *self.peek() == TokenKind::AmpAmp {
            let span = self.span(); self.advance();
            let rhs = self.parse_bitor();
            lhs = Expr::BinOp(Box::new(lhs), BinOp::And, Box::new(rhs), span);
        }
        lhs
    }

    fn parse_bitor(&mut self) -> Expr {
        let mut lhs = self.parse_bitxor();
        while *self.peek() == TokenKind::Pipe {
            let span = self.span(); self.advance();
            let rhs = self.parse_bitxor();
            lhs = Expr::BinOp(Box::new(lhs), BinOp::BitOr, Box::new(rhs), span);
        }
        lhs
    }

    fn parse_bitxor(&mut self) -> Expr {
        let mut lhs = self.parse_bitand();
        while *self.peek() == TokenKind::Caret {
            let span = self.span(); self.advance();
            let rhs = self.parse_bitand();
            lhs = Expr::BinOp(Box::new(lhs), BinOp::BitXor, Box::new(rhs), span);
        }
        lhs
    }

    fn parse_bitand(&mut self) -> Expr {
        let mut lhs = self.parse_comparison();
        // Only treat & as bitwise AND in binary position (not prefix &, which is handled by parse_unary)
        while *self.peek() == TokenKind::Ampersand {
            let span = self.span(); self.advance();
            let rhs = self.parse_comparison();
            lhs = Expr::BinOp(Box::new(lhs), BinOp::BitAnd, Box::new(rhs), span);
        }
        lhs
    }

    fn parse_comparison(&mut self) -> Expr {
        let mut lhs = self.parse_additive();
        loop {
            let op = match self.peek() {
                TokenKind::EqEq   => BinOp::Eq,
                TokenKind::BangEq => BinOp::Ne,
                TokenKind::Lt     => BinOp::Lt,
                TokenKind::LtEq   => BinOp::Le,
                TokenKind::Gt     => BinOp::Gt,
                TokenKind::GtEq   => BinOp::Ge,
                _ => break,
            };
            let span = self.span(); self.advance();
            let rhs = self.parse_additive();
            lhs = Expr::BinOp(Box::new(lhs), op, Box::new(rhs), span);
        }
        lhs
    }

    fn parse_additive(&mut self) -> Expr {
        let mut lhs = self.parse_shift();
        loop {
            let op = match self.peek() {
                TokenKind::Plus  => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            let span = self.span(); self.advance();
            let rhs = self.parse_shift();
            lhs = Expr::BinOp(Box::new(lhs), op, Box::new(rhs), span);
        }
        lhs
    }

    fn parse_shift(&mut self) -> Expr {
        let mut lhs = self.parse_multiplicative();
        loop {
            let op = match self.peek() {
                TokenKind::LtLt => BinOp::Shl,
                TokenKind::GtGt => BinOp::Shr,
                _ => break,
            };
            let span = self.span(); self.advance();
            let rhs = self.parse_multiplicative();
            lhs = Expr::BinOp(Box::new(lhs), op, Box::new(rhs), span);
        }
        lhs
    }

    fn parse_multiplicative(&mut self) -> Expr {
        let mut lhs = self.parse_unary();
        loop {
            let op = match self.peek() {
                TokenKind::Star    => BinOp::Mul,
                TokenKind::Slash   => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            let span = self.span(); self.advance();
            let rhs = self.parse_unary();
            lhs = Expr::BinOp(Box::new(lhs), op, Box::new(rhs), span);
        }
        lhs
    }

    fn parse_unary(&mut self) -> Expr {
        let span = self.span();
        match self.peek() {
            TokenKind::KwAwait => {
                self.advance();
                Expr::Await(Box::new(self.parse_unary()), span)
            }
            TokenKind::Minus => { self.advance(); Expr::Unary(UnaryOp::Neg,    Box::new(self.parse_unary()), span) }
            TokenKind::Bang  => { self.advance(); Expr::Unary(UnaryOp::Not,    Box::new(self.parse_unary()), span) }
            TokenKind::Star  => { self.advance(); Expr::Unary(UnaryOp::Deref,  Box::new(self.parse_unary()), span) }
            TokenKind::Tilde => { self.advance(); Expr::Unary(UnaryOp::BitNot, Box::new(self.parse_unary()), span) }
            TokenKind::Ampersand => {
                self.advance();
                let is_mut = self.eat(&TokenKind::KwMut);
                let inner = self.parse_unary();
                Expr::Ref(is_mut, Box::new(inner), span)
            }
            TokenKind::PlusPlus => {
                self.advance();
                let inner = self.parse_unary();
                Expr::CompoundAssign(Box::new(inner), BinOp::Add, Box::new(Expr::IntLit(1, span)), span)
            }
            TokenKind::MinusMinus => {
                self.advance();
                let inner = self.parse_unary();
                Expr::CompoundAssign(Box::new(inner), BinOp::Sub, Box::new(Expr::IntLit(1, span)), span)
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_primary();
        loop {
            let span = self.span();
            match self.peek() {
                TokenKind::Dot => {
                    self.advance();
                    let name = match self.peek().clone() {
                        TokenKind::Ident(s) => { self.advance(); s }
                        // tuple index: expr.0
                        TokenKind::IntLit(n) => { self.advance(); n.to_string() }
                        other => self.error(format!("expected field name after '.', got {}", other.describe())),
                    };
                    if *self.peek() == TokenKind::LParen {
                        // method call
                        self.advance();
                        let mut args = Vec::new();
                        while *self.peek() != TokenKind::RParen {
                            if !args.is_empty() { self.expect(&TokenKind::Comma); }
                            args.push(self.parse_expr());
                        }
                        self.expect(&TokenKind::RParen);
                        expr = Expr::MethodCall(Box::new(expr), name, args, span);
                    } else {
                        expr = Expr::FieldAccess(Box::new(expr), name, span);
                    }
                }
                TokenKind::LBracket => {
                    self.advance();
                    let idx = self.parse_expr();
                    self.expect(&TokenKind::RBracket);
                    expr = Expr::Index(Box::new(expr), Box::new(idx), span);
                }
                TokenKind::Question => {
                    self.advance();
                    expr = Expr::Question(Box::new(expr), span);
                }
                TokenKind::KwAs => {
                    self.advance();
                    let ty = self.parse_type();
                    expr = Expr::Cast(Box::new(expr), ty, span);
                }
                TokenKind::PlusPlus => {
                    self.advance();
                    expr = Expr::CompoundAssign(Box::new(expr), BinOp::Add, Box::new(Expr::IntLit(1, span)), span);
                }
                TokenKind::MinusMinus => {
                    self.advance();
                    expr = Expr::CompoundAssign(Box::new(expr), BinOp::Sub, Box::new(Expr::IntLit(1, span)), span);
                }
                _ => break,
            }
        }
        expr
    }

    fn parse_primary(&mut self) -> Expr {
        let span = self.span();
        match self.peek().clone() {
            TokenKind::IntLit(n)    => { self.advance(); Expr::IntLit(n, span) }
            TokenKind::FloatLit(f)  => { self.advance(); Expr::FloatLit(f, span) }
            TokenKind::StringLit(s) => { self.advance(); Expr::StringLit(s, span) }
            TokenKind::KwTrue       => { self.advance(); Expr::BoolLit(true,  span) }
            TokenKind::KwFalse      => { self.advance(); Expr::BoolLit(false, span) }
            TokenKind::KwSelf       => { self.advance(); Expr::SelfExpr(span) }

            TokenKind::KwLambda => {
                // closure: lambda(params) { block } — captures are moved automatically
                self.advance();
                self.expect(&TokenKind::LParen);
                let mut params = Vec::new();
                while *self.peek() != TokenKind::RParen {
                    if !params.is_empty() { self.expect(&TokenKind::Comma); }
                    let ty = if self.peek_is_type() { Some(self.parse_type()) } else { None };
                    let name = self.parse_ident();
                    params.push(ClosureParam { name, ty });
                }
                self.expect(&TokenKind::RParen);
                let body = self.parse_block();
                Expr::Closure(params, body, span)
            }

            TokenKind::LBracket => {
                // array literal: [expr, expr, ...]
                self.advance();
                let mut elems = Vec::new();
                while *self.peek() != TokenKind::RBracket {
                    if !elems.is_empty() { self.expect(&TokenKind::Comma); }
                    elems.push(self.parse_expr());
                }
                self.expect(&TokenKind::RBracket);
                Expr::Array(elems, span)
            }

            TokenKind::LParen => {
                // C-style cast: (type)expr
                if self.peek_is_c_cast() {
                    self.advance(); // (
                    let ty = self.parse_type();
                    self.expect(&TokenKind::RParen);
                    let expr = self.parse_unary();
                    return Expr::Cast(Box::new(expr), ty, span);
                }
                self.advance();
                if *self.peek() == TokenKind::RParen {
                    self.advance();
                    return Expr::Tuple(Vec::new(), span); // unit ()
                }
                let first = self.parse_expr();
                if *self.peek() == TokenKind::Comma {
                    // tuple
                    let mut elems = vec![first];
                    while self.eat(&TokenKind::Comma) {
                        if *self.peek() == TokenKind::RParen { break; }
                        elems.push(self.parse_expr());
                    }
                    self.expect(&TokenKind::RParen);
                    Expr::Tuple(elems, span)
                } else {
                    self.expect(&TokenKind::RParen);
                    first
                }
            }

            TokenKind::Ident(name) => {
                self.advance();
                if *self.peek() == TokenKind::ColonColon {
                    let mut segs = vec![name];
                    while self.eat(&TokenKind::ColonColon) {
                        segs.push(self.parse_ident());
                    }
                    let (args, is_call) = if *self.peek() == TokenKind::LParen {
                        self.advance();
                        let mut args = Vec::new();
                        while *self.peek() != TokenKind::RParen {
                            if !args.is_empty() { self.expect(&TokenKind::Comma); }
                            args.push(self.parse_expr());
                        }
                        self.expect(&TokenKind::RParen);
                        (args, true)
                    } else { (Vec::new(), false) };
                    Expr::Path(segs, args, is_call, span)
                } else if *self.peek() == TokenKind::LParen {
                    self.advance();
                    let mut args = Vec::new();
                    while *self.peek() != TokenKind::RParen {
                        if !args.is_empty() { self.expect(&TokenKind::Comma); }
                        args.push(self.parse_expr());
                    }
                    self.expect(&TokenKind::RParen);
                    if self.extern_fns.contains(&name) {
                        Expr::ExternCall(name, args, span)
                    } else {
                        Expr::Call(name, args, span)
                    }
                } else if *self.peek() == TokenKind::LBrace && self.known_types.contains(&name) {
                    self.advance();
                    let mut fields = Vec::new();
                    while *self.peek() != TokenKind::RBrace {
                        if !fields.is_empty() { self.expect(&TokenKind::Comma); }
                        if *self.peek() == TokenKind::RBrace { break; }   // trailing comma
                        let fname = self.parse_ident();
                        self.expect(&TokenKind::Colon);
                        let val = self.parse_expr();
                        fields.push((fname, val));
                    }
                    self.expect(&TokenKind::RBrace);
                    Expr::StructLit(name, fields, span)
                } else {
                    Expr::Ident(name, span)
                }
            }

            other => self.error(format!("expected expression, got {}", other.describe())),
        }
    }

    // ── types ─────────────────────────────────────────────────────────────────

    fn parse_type(&mut self) -> Type {
        match self.peek().clone() {
            TokenKind::KwInt   => { self.advance(); Type::Named("i64".to_string()) }
            TokenKind::KwFloat => { self.advance(); Type::Named("f64".to_string()) }
            TokenKind::KwBool  => { self.advance(); Type::Named("bool".to_string()) }
            TokenKind::KwVoid  => { self.advance(); Type::Named("()".to_string()) }
            TokenKind::KwChar   => { self.advance(); Type::Named("i8".to_string()) }
            TokenKind::KwString => { self.advance(); Type::Named("String".to_string()) }
            TokenKind::KwImpl   => { self.advance(); Type::ImplTrait(self.parse_ident()) }
            TokenKind::Star    => { self.advance(); Type::Ptr(Box::new(self.parse_type())) }
            TokenKind::Ampersand => {
                self.advance();
                if self.eat(&TokenKind::KwMut) {
                    Type::MutRef(Box::new(self.parse_type()))
                } else {
                    Type::Ref(Box::new(self.parse_type()))
                }
            }
            TokenKind::LParen => {
                self.advance();
                if *self.peek() == TokenKind::RParen { self.advance(); return Type::Named("()".to_string()); }
                let mut types = vec![self.parse_type()];
                while self.eat(&TokenKind::Comma) { types.push(self.parse_type()); }
                self.expect(&TokenKind::RParen);
                Type::Tuple(types)
            }
            TokenKind::LBracket => {
                self.advance();
                let inner = self.parse_type();
                let n = if self.eat(&TokenKind::Semicolon) {
                    if let TokenKind::IntLit(n) = self.peek().clone() {
                        self.advance(); Some(n as usize)
                    } else { self.error(format!("expected array size, got {}", self.peek().describe())) }
                } else { None };
                self.expect(&TokenKind::RBracket);
                Type::Array(Box::new(inner), n)
            }
            TokenKind::Ident(name) => {
                self.advance();
                // generic: Name<T, U>
                if *self.peek() == TokenKind::Lt {
                    self.advance();
                    let mut args = Vec::new();
                    while *self.peek() != TokenKind::Gt {
                        if !args.is_empty() { self.expect(&TokenKind::Comma); }
                        args.push(self.parse_type());
                    }
                    self.expect(&TokenKind::Gt);
                    Type::Generic(name, args)
                } else {
                    Type::Named(name)
                }
            }
            other => self.error(format!("expected type, got {}", other.describe())),
        }
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    // Detect C-style cast: (type) — peek past ( to see if type + ) follows
    fn peek_is_c_cast(&self) -> bool {
        let i = (self.pos + 1).min(self.tokens.len() - 1);
        let is_type = match &self.tokens[i].kind {
            TokenKind::KwInt | TokenKind::KwFloat | TokenKind::KwBool | TokenKind::KwVoid => true,
            TokenKind::Ident(name) => self.known_types.contains(name),
            _ => false,
        };
        if !is_type { return false; }
        let j = (i + 1).min(self.tokens.len() - 1);
        matches!(&self.tokens[j].kind, TokenKind::RParen)
    }

    fn peek_is_type(&self) -> bool {
        match self.peek() {
            TokenKind::KwInt | TokenKind::KwFloat | TokenKind::KwBool | TokenKind::KwVoid |
            TokenKind::KwChar | TokenKind::KwString | TokenKind::KwImpl |
            TokenKind::Star | TokenKind::Ampersand | TokenKind::LBracket => true,
            TokenKind::Ident(name) => {
                if !self.known_types.contains(name) { return false; }
                match self.peek2() {
                    TokenKind::Ident(_) => true,
                    TokenKind::Lt       => self.peek_past_generic(),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    // Scan past balanced <...> starting at pos+1 and check if an Ident follows.
    fn peek_past_generic(&self) -> bool {
        self.peek_past_generic_from(self.pos + 1)
    }

    fn peek_past_generic_from(&self, start: usize) -> bool {
        let mut i = start;
        let mut depth = 0usize;
        loop {
            if i >= self.tokens.len() { return false; }
            match &self.tokens[i].kind {
                TokenKind::Lt  => depth += 1,
                TokenKind::Gt  => {
                    if depth == 0 { return false; }
                    depth -= 1;
                    if depth == 0 {
                        let j = i + 1;
                        return j < self.tokens.len() &&
                            matches!(&self.tokens[j].kind, TokenKind::Ident(_));
                    }
                }
                TokenKind::Eof => return false,
                _ => {}
            }
            i += 1;
        }
    }

    // peek_is_type / peek_is_tuple_type_decl variants that look one token ahead (past mut/mutable)
    fn peek_is_type2(&self) -> bool {
        match &self.tokens[(self.pos + 1).min(self.tokens.len() - 1)].kind {
            TokenKind::KwInt | TokenKind::KwFloat | TokenKind::KwBool | TokenKind::KwVoid |
            TokenKind::KwChar | TokenKind::KwString | TokenKind::Star | TokenKind::Ampersand | TokenKind::LBracket => true,
            TokenKind::Ident(name) => {
                if !self.known_types.contains(name) { return false; }
                let j = (self.pos + 2).min(self.tokens.len() - 1);
                match &self.tokens[j].kind {
                    TokenKind::Ident(_) => true,
                    TokenKind::Lt       => self.peek_past_generic_from(j),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn peek_is_tuple_type_decl2(&self) -> bool {
        let i = (self.pos + 1).min(self.tokens.len() - 1);
        if self.tokens[i].kind != TokenKind::LParen { return false; }
        let mut depth = 0usize;
        let mut j = i;
        while j < self.tokens.len() {
            match &self.tokens[j].kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        let k = (j + 1).min(self.tokens.len() - 1);
                        return matches!(&self.tokens[k].kind, TokenKind::Ident(_));
                    }
                }
                TokenKind::Eof => return false,
                _ => {}
            }
            j += 1;
        }
        false
    }

    // Scan past balanced parens to see if (type, type) name = ... pattern
    fn peek_is_tuple_type_decl(&self) -> bool {
        if *self.peek() != TokenKind::LParen { return false; }
        let mut depth = 0usize;
        let mut i = self.pos;
        while i < self.tokens.len() {
            match &self.tokens[i].kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        let j = (i + 1).min(self.tokens.len() - 1);
                        return matches!(&self.tokens[j].kind, TokenKind::Ident(_));
                    }
                }
                TokenKind::Eof => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    fn parse_ident(&mut self) -> String {
        match self.peek().clone() {
            TokenKind::Ident(name) => { self.advance(); name }
            other => self.error(format!("expected identifier, got {}", other.describe())),
        }
    }
}
