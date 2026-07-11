use crate::ast::Span;

/// A compile-time diagnostic with a source location.
///
/// The lexer and parser raise these via [`raise`], which unwinds with the
/// error as a typed panic payload — the same mechanism rustc uses for fatal
/// diagnostics. [`catch`] at the pipeline boundary turns the unwind back
/// into a `Result`, so parse code stays free of `Result` plumbing while
/// callers still get a structured error instead of a panic dump.
#[derive(Debug)]
pub struct CompileError {
    pub span: Span,
    pub message: String,
}

impl CompileError {
    /// Render as `file:line:col: error: message` followed by the offending
    /// source line and a caret pointing at the column.
    pub fn render(&self, file: &str, source: &str) -> String {
        let (line, col) = line_col(source, self.span.start);
        let mut out = format!("{}:{}:{}: error: {}\n", file, line, col, self.message);
        if let Some(text) = source.lines().nth(line - 1) {
            out.push_str(text);
            out.push('\n');
            // keep tabs as tabs so the caret lines up in a terminal
            for c in text.chars().take(col - 1) {
                out.push(if c == '\t' { '\t' } else { ' ' });
            }
            out.push('^');
        }
        out
    }
}

/// Abort compilation with an error at `span`. Never returns.
pub fn raise(span: Span, message: String) -> ! {
    std::panic::panic_any(CompileError { span, message });
}

/// Run a pipeline stage, converting a raised [`CompileError`] into `Err`.
/// Any other panic is propagated unchanged.
pub fn catch<T>(f: impl FnOnce() -> T + std::panic::UnwindSafe) -> Result<T, CompileError> {
    install_quiet_hook();
    match std::panic::catch_unwind(f) {
        Ok(v) => Ok(v),
        Err(payload) => match payload.downcast::<CompileError>() {
            Ok(e) => Err(*e),
            Err(other) => std::panic::resume_unwind(other),
        },
    }
}

/// Suppress the default "thread panicked" banner for CompileError unwinds —
/// they are ordinary diagnostics, not bugs. All other panics keep the
/// default hook behaviour.
fn install_quiet_hook() {
    static HOOK: std::sync::Once = std::sync::Once::new();
    HOOK.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if info.payload().downcast_ref::<CompileError>().is_none() {
                prev(info);
            }
        }));
    });
}

/// Byte offset → 1-based (line, column). Columns count characters, not bytes.
fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let col = before.chars().rev().take_while(|&c| c != '\n').count() + 1;
    (line, col)
}
