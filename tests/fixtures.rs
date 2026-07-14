//! Transpile every fixture in tests/fixtures/ and verify the emitted Rust
//! actually compiles (rustc syntax + type check, no linking).

use std::path::Path;
use std::process::Command;

fn transpile_fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {}", path.display(), e));
    crust::transpile(&source)
}

/// Compile emitted Rust with rustc as a library (allows a dead `fn main`,
/// checks everything, links nothing). Panics with rustc's stderr on failure.
fn assert_compiles(name: &str, rust_src: &str) {
    let dir = std::env::temp_dir().join(format!("crust-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rs = dir.join(format!("{}.rs", name.replace('.', "_")));
    std::fs::write(&rs, rust_src).unwrap();

    let out = Command::new("rustc")
        .args(["--edition", "2021", "--crate-type", "lib", "--emit", "metadata"])
        .arg("-A").arg("warnings")
        .arg("-o").arg(dir.join(format!("{}.rmeta", name.replace('.', "_"))))
        .arg(&rs)
        .output()
        .expect("failed to run rustc");

    if !out.status.success() {
        panic!(
            "emitted Rust for fixture '{}' does not compile:\n--- rustc stderr ---\n{}\n--- emitted Rust ---\n{}",
            name,
            String::from_utf8_lossy(&out.stderr),
            rust_src
        );
    }
}

macro_rules! fixture_compiles {
    ($test:ident, $file:literal) => {
        #[test]
        fn $test() {
            let rust = transpile_fixture($file);
            assert_compiles($file, &rust);
        }
    };
}

fixture_compiles!(add, "add.crust");
fixture_compiles!(control_flow, "control_flow.crust");
fixture_compiles!(demo, "demo.crust");
fixture_compiles!(enums, "enums.crust");
fixture_compiles!(extern_c, "extern_c.crust");
fixture_compiles!(for_loop, "for_loop.crust");
fixture_compiles!(hashmap, "hashmap.crust");
fixture_compiles!(iterators, "iterators.crust");
fixture_compiles!(match_, "match.crust");
fixture_compiles!(showcase, "showcase.crust");
fixture_compiles!(strings, "strings.crust");
fixture_compiles!(structs, "structs.crust");
fixture_compiles!(switch, "switch.crust");
fixture_compiles!(traits, "traits.crust");
fixture_compiles!(threads, "threads.crust");
fixture_compiles!(pointers, "pointers.crust");
fixture_compiles!(derive, "derive.crust");

// async needs tokio to compile, which rustc alone can't provide —
// assert on the emitted source instead.
#[test]
fn async_await() {
    let rust = transpile_fixture("async.crust");
    assert!(rust.contains("async fn fetch()"), "async function not emitted:\n{rust}");
    assert!(rust.contains("(fetch()).await"), "prefix await not desugared to .await:\n{rust}");
    assert!(rust.contains("#[tokio::main]\nasync fn main()"), "async main missing tokio attribute:\n{rust}");
}

#[test]
fn if_auto_and_if_let_equivalent() {
    // canonical: if (auto Pattern = expr); Rust's bare `if let` is the alias
    let auto_form = crust::transpile(
        "void f(Option<int> maybe)\n{\n    if (auto Some(n) = maybe)\n    {\n        println(\"{}\", n);\n    }\n}\n");
    let let_form = crust::transpile(
        "void f(Option<int> maybe)\n{\n    if let Some(n) = maybe\n    {\n        println(\"{}\", n);\n    }\n}\n");
    assert_eq!(auto_form, let_form);
    assert!(auto_form.contains("if let Some(n) = maybe"));

    let auto_while = crust::transpile(
        "void f(&mut Vec<int> stack)\n{\n    while (auto Some(v) = stack.pop())\n    {\n        println(\"{}\", v);\n    }\n}\n");
    assert!(auto_while.contains("while let Some(v) ="));
}

#[test]
fn switch_and_match_equivalent() {
    // switch is the canonical spelling; match is the Rust-flavored alias
    let sw = crust::transpile(concat!(
        "int f(Result<int, string> r)\n{\n",
        "    switch (r)\n    {\n",
        "        case Ok(v): { return v; }\n",
        "        case Err(_): { return 0; }\n",
        "    }\n}\n",
    ));
    let m = crust::transpile(concat!(
        "int f(Result<int, string> r)\n{\n",
        "    match r\n    {\n",
        "        Ok(v): { return v; }\n",
        "        Err(_): { return 0; }\n",
        "    }\n}\n",
    ));
    assert_eq!(sw, m);
    assert!(sw.contains("Ok(v) =>"), "destructuring case not emitted:\n{sw}");
}

#[test]
fn for_in_all_forms_equivalent() {
    // canonical C++ range-for spelling, with auto implied or explicit,
    // plus the 'in' alias and the bare Rust-style form
    let colon      = crust::transpile("void main()\n{\n    for (i : 0..5) { println(\"{}\", i); }\n}\n");
    let colon_auto = crust::transpile("void main()\n{\n    for (auto i : 0..5) { println(\"{}\", i); }\n}\n");
    let parens_in  = crust::transpile("void main()\n{\n    for (i in 0..5) { println(\"{}\", i); }\n}\n");
    let bare       = crust::transpile("void main()\n{\n    for i in 0..5 { println(\"{}\", i); }\n}\n");
    assert_eq!(colon, colon_auto);
    assert_eq!(colon, parens_in);
    assert_eq!(colon, bare);
    assert!(colon.contains("for i in 0..5"));
}

#[test]
fn auto_and_let_equivalent() {
    let with_let  = crust::transpile("void main()\n{\n    let x = 42;\n    let mut y = x + 1;\n}\n");
    let with_auto = crust::transpile("void main()\n{\n    auto x = 42;\n    auto mut y = x + 1;\n}\n");
    assert_eq!(with_let, with_auto);
    // const by default; mut only when declared
    assert!(with_let.contains("let x = 42;"));
    assert!(with_let.contains("let mut y"));
}

#[test]
fn const_by_default() {
    let rust = crust::transpile(concat!(
        "void main()\n{\n",
        "    int x = 1;\n",
        "    mutable int y = 2;\n",
        "    mutable Vec<int> v = vec();\n",
        "    auto mutable z = 3;\n",
        "}\n",
    ));
    assert!(rust.contains("let x: i64 = 1;"), "x must be immutable:\n{rust}");
    assert!(rust.contains("let mut y: i64 = 2;"), "y must be mutable:\n{rust}");
    assert!(rust.contains("let mut v: Vec<i64>"), "v must be mutable:\n{rust}");
    assert!(rust.contains("let mut z = 3;"), "z must be mutable:\n{rust}");
}

#[test]
fn derive_all_forms_equivalent() {
    // #derive is canonical; @derive and Rust's #[derive(...)] are aliases
    let pound = crust::transpile("#derive(Debug, Clone)\nstruct P\n{\n    int x;\n}\n");
    let at    = crust::transpile("@derive(Debug, Clone)\nstruct P\n{\n    int x;\n}\n");
    let rust  = crust::transpile("#[derive(Debug, Clone)]\nstruct P\n{\n    int x;\n}\n");
    assert_eq!(pound, at);
    assert_eq!(pound, rust);
    assert!(pound.contains("#[derive(Debug, Clone)]"));
}

#[test]
fn hello_world_readme_example() {
    let rust = crust::transpile(concat!(
        "string greet(string name)\n{\n",
        "    return \"Hello, \" + name + \"!\";\n}\n\n",
        "void main()\n{\n",
        "    string msg = greet(\"world\");\n",
        "    println(\"{}\", msg);\n}\n",
    ));
    assert!(rust.contains("greet(String::from(\"world\"))"),
        "string literal arg not coerced to String:\n{rust}");
    assert_compiles("hello_readme", &rust);
}

#[test]
fn main_signature_forms() {
    // bare main gets no argv binding (and thus no unused-variable warning)
    let bare = crust::transpile("void main()\n{\n    println(\"hi\");\n}\n");
    assert!(!bare.contains("std::env::args()"),
        "bare main must not collect argv:\n{bare}");

    // explicit parameter picks the binding name; Rust main stays parameterless
    let named = crust::transpile("void main(Vec<string> argv)\n{\n    println(\"{}\", argv.len());\n}\n");
    assert!(named.contains("let argv: Vec<String> = std::env::args().collect();"),
        "declared arg name not used:\n{named}");
    assert!(named.contains("fn main()\n"), "Rust main must stay parameterless:\n{named}");
    assert_compiles("main_named_args", &named);

    // int main: the return value becomes the process exit code
    let coded = crust::transpile("int main(Vec<string> args)\n{\n    return 2;\n}\n");
    assert!(coded.contains("std::process::exit(__crust_main(args) as i32)"),
        "exit-code wrapper missing:\n{coded}");
    assert!(coded.contains("fn __crust_main(args: Vec<String>) -> i64"),
        "wrapped main missing:\n{coded}");
    assert_compiles("main_exit_code", &coded);
}
