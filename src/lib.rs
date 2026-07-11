pub mod ast;
pub mod codegen;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod sema;

/// Run the full pipeline: crust source text in, Rust source text out.
/// Unwinds with a `CompileError` payload on invalid input — use
/// [`transpile_result`] to get it as a `Result` instead.
pub fn transpile(source: &str) -> String {
    let tokens = lexer::tokenize(source);
    let ast = parser::parse(tokens);
    let (ast, sema) = sema::check(ast);
    codegen::emit(ast, &sema)
}

/// Like [`transpile`], but catches compile errors and returns them.
pub fn transpile_result(source: &str) -> Result<String, error::CompileError> {
    error::catch(|| transpile(source))
}
