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
fn auto_and_let_equivalent() {
    let with_let  = crust::transpile("function main()\n{\n    let x = 42;\n    let mut y = x + 1;\n}\n");
    let with_auto = crust::transpile("function main()\n{\n    auto x = 42;\n    auto mut y = x + 1;\n}\n");
    assert_eq!(with_let, with_auto);
    assert!(with_let.contains("let mut x = 42;"));
}

#[test]
fn derive_both_forms_equivalent() {
    let at = crust::transpile("@derive(Debug, Clone)\nstruct P\n{\n    int x;\n}\n");
    let hash = crust::transpile("#[derive(Debug, Clone)]\nstruct P\n{\n    int x;\n}\n");
    assert_eq!(at, hash);
    assert!(at.contains("#[derive(Debug, Clone)]"));
}

#[test]
fn hello_world_readme_example() {
    let rust = crust::transpile(concat!(
        "function greet(string name): string\n{\n",
        "    return \"Hello, \" + name + \"!\";\n}\n\n",
        "function main()\n{\n",
        "    string msg = greet(\"world\");\n",
        "    println(\"{}\", msg);\n}\n",
    ));
    assert!(rust.contains("greet(String::from(\"world\"))"),
        "string literal arg not coerced to String:\n{rust}");
    assert_compiles("hello_readme", &rust);
}
