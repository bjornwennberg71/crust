//! Invalid programs must produce a structured CompileError with a correct
//! source location — never a raw panic dump.

fn error_for(source: &str) -> (usize, usize, String) {
    match crust::transpile_result(source) {
        Ok(rust) => panic!("expected a compile error, got successful output:\n{rust}"),
        Err(e) => {
            let rendered = e.render("test.cru", source);
            // first line is "file:line:col: error: message"
            let first = rendered.lines().next().unwrap().to_string();
            let mut parts = first.splitn(4, ':');
            parts.next(); // file
            let line = parts.next().unwrap().parse().unwrap();
            let col = parts.next().unwrap().parse().unwrap();
            (line, col, first)
        }
    }
}

#[test]
fn missing_semicolon_points_at_next_token() {
    let (line, col, msg) = error_for("void main()\n{\n    int x = 42\n    println(\"{}\", x);\n}\n");
    assert_eq!((line, col), (4, 5), "wrong location: {msg}");
    assert!(msg.contains("expected ';'"), "unexpected message: {msg}");
}

#[test]
fn unexpected_character_is_an_error() {
    let (line, col, msg) = error_for("void main()\n{\n    int \u{20ac} = 1;\n}\n");
    assert_eq!((line, col), (3, 9), "wrong location: {msg}");
    assert!(msg.contains("unexpected character"), "unexpected message: {msg}");
}

#[test]
fn bad_top_level_item_names_the_alternatives() {
    let (line, col, msg) = error_for("return 1;\n");
    assert_eq!((line, col), (1, 1), "wrong location: {msg}");
    assert!(msg.contains("expected an item"), "unexpected message: {msg}");
}

#[test]
fn match_arm_without_separator() {
    let (_, _, msg) = error_for("void main()\n{\n    match x { 1 { return; } }\n}\n");
    assert!(msg.contains("after match pattern"), "unexpected message: {msg}");
}

#[test]
fn trailing_return_type_is_an_error() {
    // old crust spelling — the return type goes in front of the name now
    let (_, _, msg) = error_for("int add(int a, int b): int\n{\n    return a + b;\n}\n");
    assert!(msg.contains("expected '{'"), "unexpected message: {msg}");
}

#[test]
fn valid_program_still_transpiles_through_result_api() {
    let rust = crust::transpile_result("void main()\n{\n    println(\"ok\");\n}\n").unwrap();
    assert!(rust.contains("println!"));
}

#[test]
fn main_parameter_must_be_vec_string() {
    let (line, col, msg) = error_for("void main(int x)\n{\n}\n");
    assert_eq!((line, col), (1, 11), "wrong location: {msg}");
    assert!(msg.contains("Vec<string>"), "unexpected message: {msg}");
}

#[test]
fn main_return_type_must_be_int_or_void() {
    let (_, _, msg) = error_for("string main()\n{\n    return \"x\";\n}\n");
    assert!(msg.contains("exit code"), "unexpected message: {msg}");
}
