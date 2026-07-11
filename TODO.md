# Feature requests / backlog

Things worth doing if crust development picks up again. Roughly ordered by
value; none are commitments — the experiment is concluded (see README).

## Language

- **C-string support for extern "C"** — passing a dynamic `string` to a
  `*char` parameter currently requires a manual embedded NUL and cast:
  `format("...\0", x).as_ptr() as *i8` (see `examples/pci_probe.cru`).
  Either a `cstring` type or automatic conversion at extern call sites.
  String literals already convert automatically.
- **Clone elision in the sema pass** — `for x in v` and `.iter()` chains
  clone the collection to get value semantics. Sema has the signature
  tables now; ownership analysis to skip the clone when the source is not
  used again is the natural next step.
- **Struct destructuring in patterns** — `let Point { x, y } = p;`
- **Lifetimes** — functions needing explicit lifetime annotations are not
  expressible; decide whether to hide or expose them.
- **`continue` in C-style `for` loops** — currently skips the update
  expression (documented footgun in the README).

## Tooling / codebase

- **Reformat the transpiler's own Rust source to Allman style** — the
  emitted Rust and all crust code are Allman; `src/*.rs` is still standard
  rustfmt style. Deferred 2026-07-11 ("don't want to bother with that
  now") — would need a rustfmt.toml (`brace_style = "AlwaysNextLine"`) and
  a one-time reformat commit.
- **Error recovery in the parser** — diagnostics are good now
  (file:line:col + caret) but the parser stops at the first error;
  reporting several errors per run would help larger files.
- **LSP server / editor support beyond Emacs** — crust-mode exists for
  Emacs; a minimal language server would unlock VS Code and others.
