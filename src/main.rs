use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut emit_rs = false;
    let mut release = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--version" | "-V" => {
                println!("crust {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--help" | "-h" => {
                println!("crust {} — a systems language with C syntax, backed by Rust", env!("CARGO_PKG_VERSION"));
                println!();
                println!("USAGE:");
                println!("    crust [OPTIONS] <file.cru|file.crust>");
                println!();
                println!("OPTIONS:");
                println!("    -o <output>    Output binary name (default: input filename stem)");
                println!("    --release      Optimized build (cargo --release)");
                println!("    --emit-rs      Print generated Rust to stdout, do not compile");
                println!("    --version, -V  Print version");
                println!("    --help, -h     Print this help");
                println!();
                println!("DEPENDENCIES:");
                println!("    Add a crust.toml next to your .cru file:");
                println!("        [dependencies]");
                println!("        serde = \"1.0\"");
                println!();
                println!("EXAMPLES:");
                println!("    crust hello.cru                 # compile");
                println!("    crust --release hello.cru       # optimized");
                println!("    crust -o /tmp/hello hello.cru   # custom output path");
                println!("    crust --emit-rs hello.cru       # inspect generated Rust");
                println!();
                println!("Build artifacts are cached in .crust/<name>/ next to your source file.");
                println!("Add .crust/ to your .gitignore.");
                println!();
                println!("https://github.com/bjornwennberg71/crust/issues");
                return;
            }
            "--emit-rs" => emit_rs = true,
            "--release" | "-O" => release = true,
            "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: -o requires an argument");
                    std::process::exit(1);
                }
                output = Some(PathBuf::from(&args[i]));
            }
            arg => {
                let ext = PathBuf::from(arg)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();

                if !arg.starts_with('-') && (ext == "cru" || ext == "crust") {
                    if input.is_some() {
                        eprintln!("error: multiple input files specified");
                        std::process::exit(1);
                    }
                    input = Some(PathBuf::from(arg));
                } else {
                    eprintln!("warning: unknown flag {:?} ignored", arg);
                }
            }
        }
        i += 1;
    }

    let input = input.unwrap_or_else(|| {
        eprintln!("usage: crust [--release] [--emit-rs] [-o output] <file.crust|file.cru>");
        std::process::exit(1);
    });

    let source = std::fs::read_to_string(&input).unwrap_or_else(|e| {
        eprintln!("error: cannot read {}: {e}", input.display());
        std::process::exit(1);
    });

    let rust_src = crust::transpile_result(&source).unwrap_or_else(|e| {
        eprintln!("{}", e.render(&input.display().to_string(), &source));
        std::process::exit(1);
    });

    if emit_rs {
        print!("{rust_src}");
        return;
    }

    let stem = input.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let source_dir = input.parent().unwrap_or(Path::new("."));

    // Persistent build directory: <source_dir>/.crust/<stem>/
    let build_dir = source_dir.join(".crust").join(&stem);
    let src_dir = build_dir.join("src");

    std::fs::create_dir_all(&src_dir).unwrap_or_else(|e| {
        eprintln!("error: cannot create build dir {}: {e}", src_dir.display());
        std::process::exit(1);
    });

    // Write Cargo.toml — only on first build; user can edit it afterwards.
    // If a crust.toml exists next to the source file, merge its [dependencies].
    let cargo_toml_path = build_dir.join("Cargo.toml");
    let mut deps = load_crust_toml(source_dir);
    // async programs need an executor; crust provides tokio automatically
    if rust_src.contains("#[tokio::main]") && !deps.contains("tokio") {
        deps.push_str("tokio = { version = \"1\", features = [\"full\"] }\n");
    }
    write_cargo_toml(&cargo_toml_path, &stem, &deps);

    // Write the generated Rust source.
    let main_rs_path = src_dir.join("main.rs");
    std::fs::write(&main_rs_path, &rust_src).unwrap_or_else(|e| {
        eprintln!("error: cannot write {}: {e}", main_rs_path.display());
        std::process::exit(1);
    });

    // Run cargo build.
    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }
    cmd.current_dir(&build_dir);

    let status = cmd.status().unwrap_or_else(|e| {
        eprintln!("error: cannot run cargo: {e}");
        std::process::exit(1);
    });

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    // Copy binary to the output location.
    let profile = if release { "release" } else { "debug" };
    let built = build_dir.join("target").join(profile).join(&stem);
    let dest = output.unwrap_or_else(|| PathBuf::from(&stem));

    std::fs::copy(&built, &dest).unwrap_or_else(|e| {
        eprintln!("error: cannot copy binary to {}: {e}", dest.display());
        std::process::exit(1);
    });
}

// Read [dependencies] from a crust.toml next to the source file, if present.
fn load_crust_toml(source_dir: &Path) -> String {
    let path = source_dir.join("crust.toml");
    let Ok(text) = std::fs::read_to_string(&path) else { return String::new(); };

    // Extract everything from [dependencies] to the next section (or EOF).
    let mut in_deps = false;
    let mut deps = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if in_deps && !trimmed.is_empty() {
            deps.push_str(line);
            deps.push('\n');
        }
    }
    deps
}

// Write Cargo.toml, always regenerating the [dependencies] block from crust.toml
// while preserving any [profile.*] or other sections the user may have added.
fn write_cargo_toml(path: &Path, name: &str, deps: &str) {
    // If the file already exists, preserve everything below [dependencies].
    // We only manage the header and [dependencies] ourselves.
    let header = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"
    );
    let deps_section = format!("\n[dependencies]\n{deps}");

    // Preserve [profile.*] and any other user-added sections.
    let extra = if let Ok(existing) = std::fs::read_to_string(path) {
        let mut collecting = false;
        let mut extra = String::new();
        for line in existing.lines() {
            let t = line.trim();
            if t.starts_with("[profile") || t.starts_with("[features") || t.starts_with("[patch") {
                collecting = true;
            }
            if collecting {
                extra.push_str(line);
                extra.push('\n');
            }
        }
        if !extra.is_empty() { format!("\n{extra}") } else { String::new() }
    } else {
        String::new()
    };

    let content = format!("{header}{deps_section}{extra}");
    std::fs::write(path, content).unwrap_or_else(|e| {
        eprintln!("error: cannot write Cargo.toml: {e}");
        std::process::exit(1);
    });
}
