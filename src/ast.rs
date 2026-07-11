#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub enum Item {
    Use(UseItem),
    TypeAlias(TypeAlias),
    Const(ConstItem),
    Extern(ExternBlock),
    Trait(Trait),
    Struct(Struct),
    Enum(Enum),
    Impl(ImplBlock),
    Function(Function),
}

// ── extern "C" ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ExternBlock {
    pub abi: String,
    pub functions: Vec<ExternFn>,
}

#[derive(Debug)]
pub struct ExternFn {
    pub span: Span,
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
}

// ── trait ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Trait {
    pub span: Span,
    pub pub_: bool,
    pub name: String,
    pub methods: Vec<TraitMethod>,
}

#[derive(Debug)]
pub struct TraitMethod {
    pub span: Span,
    pub name: String,
    pub self_param: Option<SelfParam>,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub body: Option<Block>,   // None = abstract (must implement), Some = default impl
}

// ── use ──────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct UseItem {
    pub path: String,   // raw path string, emitted verbatim
}

// ── type alias ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TypeAlias {
    pub span: Span,
    pub pub_: bool,
    pub name: String,
    pub ty: Type,
}

// ── const / static ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ConstItem {
    pub span: Span,
    pub pub_: bool,
    pub is_static: bool,
    pub name: String,
    pub ty: Type,
    pub value: Expr,
}

// ── struct ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Struct {
    pub span: Span,
    pub pub_: bool,
    pub derives: Vec<String>,
    pub name: String,
    pub fields: Vec<StructField>,
}

#[derive(Debug)]
pub struct StructField {
    pub span: Span,
    pub pub_: bool,
    pub name: String,
    pub ty: Type,
}

// ── enum ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Enum {
    pub span: Span,
    pub pub_: bool,
    pub derives: Vec<String>,
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug)]
pub struct EnumVariant {
    pub span: Span,
    pub name: String,
    pub fields: Vec<Type>,
}

// ── impl ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ImplBlock {
    pub span: Span,
    pub trait_name: Option<String>,   // Some("Greet") for `impl Greet for Type`
    pub type_name: String,
    pub methods: Vec<Method>,
}

#[derive(Debug)]
pub struct Method {
    pub span: Span,
    pub pub_: bool,
    pub name: String,
    pub self_param: Option<SelfParam>,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub body: Block,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelfParam {
    Value,       // self
    Ref,         // &self
    MutRef,      // &mut self
}

// ── function ─────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Function {
    pub span: Span,
    pub pub_: bool,
    pub is_async: bool,
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub body: Block,
}

#[derive(Debug)]
pub struct Param {
    pub span: Span,
    pub name: String,
    pub ty: Type,
}

// ── block / statements ────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Block {
    pub span: Span,
    pub stmts: Vec<Stmt>,
}

#[derive(Debug)]
pub enum Stmt {
    Let(LetStmt),
    Return(ReturnStmt),
    Break(Span),
    Continue(Span),
    If(IfStmt),
    While(WhileStmt),
    For(ForStmt),
    ForIn(ForInStmt),
    Match(MatchStmt),   // both `switch (x) { case p: ... }` and `match x { p: ... }`
    Unsafe(Block),
    Expr(Expr),
}

#[derive(Debug)]
pub struct LetStmt {
    pub span: Span,
    pub name: String,
    pub ty: Option<Type>,
    pub init: Option<Expr>,
}

#[derive(Debug)]
pub struct ReturnStmt {
    pub span: Span,
    pub value: Option<Expr>,
}

#[derive(Debug)]
pub struct IfStmt {
    pub span: Span,
    pub cond: IfCond,
    pub then_block: Block,
    pub else_clause: Option<ElseClause>,
}

#[derive(Debug)]
pub enum IfCond {
    Expr(Expr),
    Let(MatchPattern, Box<Expr>),   // if let Pattern = expr
}

#[derive(Debug)]
pub enum ElseClause {
    Block(Block),
    If(Box<IfStmt>),
}

#[derive(Debug)]
pub struct WhileStmt {
    pub span: Span,
    pub cond: WhileCond,
    pub body: Block,
}

#[derive(Debug)]
pub enum WhileCond {
    Expr(Expr),
    Let(MatchPattern, Box<Expr>),   // while let Pattern = expr
}

#[derive(Debug)]
pub struct ForInStmt {
    pub span: Span,
    pub var: String,
    pub iter: Expr,
    pub body: Block,
}

#[derive(Debug)]
pub struct ForStmt {
    pub span: Span,
    pub init: Option<ForInit>,
    pub cond: Option<Expr>,
    pub update: Option<Expr>,
    pub body: Block,
}

#[derive(Debug)]
pub enum ForInit {
    Decl { name: String, ty: Type, init: Expr },
    Expr(Expr),
}

#[derive(Debug)]
pub struct MatchStmt {
    pub span: Span,
    pub expr: Expr,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug)]
pub struct MatchArm {
    pub patterns: Vec<MatchPattern>,
    pub body: Block,
}

#[derive(Debug)]
pub enum MatchPattern {
    Wildcard,
    IntLit(i64),
    BoolLit(bool),
    StringLit(String),
    Ident(String),
    Path(Vec<String>, Vec<String>),   // Enum::Variant(bindings...)
    Tuple(Vec<MatchPattern>),
}

// ── expressions ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Expr {
    IntLit(i64, Span),
    FloatLit(f64, Span),
    BoolLit(bool, Span),
    StringLit(String, Span),
    Ident(String, Span),
    SelfExpr(Span),
    Tuple(Vec<Expr>, Span),
    Array(Vec<Expr>, Span),
    Index(Box<Expr>, Box<Expr>, Span),
    Assign(Box<Expr>, Box<Expr>, Span),          // lhs = rhs  (any lhs)
    CompoundAssign(Box<Expr>, BinOp, Box<Expr>, Span),  // lhs op= rhs
    FieldAccess(Box<Expr>, String, Span),
    MethodCall(Box<Expr>, String, Vec<Expr>, Span),
    StructLit(String, Vec<(String, Expr)>, Span),
    Path(Vec<String>, Vec<Expr>, bool, Span),  // is_call: true = had () in source
    Unary(UnaryOp, Box<Expr>, Span),
    BinOp(Box<Expr>, BinOp, Box<Expr>, Span),
    Call(String, Vec<Expr>, Span),
    Closure(Vec<ClosureParam>, Block, Span),
    Question(Box<Expr>, Span),                   // expr?
    Await(Box<Expr>, Span),                      // await expr — emits (expr).await
    Cast(Box<Expr>, Type, Span),                 // expr as Type
    Ref(bool, Box<Expr>, Span),                  // &expr or &mut expr
    Range(Box<Expr>, Box<Expr>, bool, Span),     // start..end (false) or start..=end (true)
    ExternCall(String, Vec<Expr>, Span),         // call to extern "C" fn — emits unsafe {}
}

#[derive(Debug)]
pub struct ClosureParam {
    pub name: String,
    pub ty: Option<Type>,
}

#[derive(Debug, Clone, Copy)]
pub enum UnaryOp {
    Neg,    // -
    Not,    // !
    Deref,  // *
    BitNot, // ~
}

#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
}

// ── types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Type {
    Named(String),
    Generic(String, Vec<Type>),   // Vec<int>, Option<int>, etc.
    Ptr(Box<Type>),
    Ref(Box<Type>),
    MutRef(Box<Type>),
    Tuple(Vec<Type>),
    Array(Box<Type>, Option<usize>),  // [T; N] or [T]
    Fn(Vec<Type>, Box<Type>),         // fn(A, B) -> C
    ImplTrait(String),                // impl TraitName — used in function params
}
