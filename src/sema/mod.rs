use crate::ast::*;
use std::collections::{HashMap, HashSet};

/// Semantic analysis pass.
///
/// Today this collects the whole-program signature facts the code generator
/// needs but cannot see while walking a single expression:
///
/// - which function parameters are crust `string` (String in Rust), so
///   string-literal arguments coerce via `String::from(...)`
/// - which parameters are `impl Trait`, which are emitted by reference,
///   so call sites must pass `&arg`
/// - which struct fields are `string`, for the same coercion in struct
///   literals
///
/// Still to come: name resolution, full type inference, and the ownership
/// analysis that lets codegen elide the defensive clones it emits today.
pub struct Sema {
    pub fn_string_params: HashMap<String, Vec<bool>>,
    pub fn_ref_params: HashMap<String, Vec<bool>>,
    pub struct_string_fields: HashMap<String, HashSet<String>>,
}

pub fn is_string_type(ty: &Type) -> bool {
    matches!(ty, Type::Named(n) if n == "String")
}

pub fn check(program: Program) -> (Program, Sema) {
    let mut sema = Sema {
        fn_string_params: HashMap::new(),
        fn_ref_params: HashMap::new(),
        struct_string_fields: HashMap::new(),
    };
    for item in &program.items {
        match item {
            Item::Function(f) => {
                let strings: Vec<bool> = f.params.iter().map(|p| is_string_type(&p.ty)).collect();
                sema.fn_string_params.insert(f.name.clone(), strings);
                let refs: Vec<bool> = f.params.iter()
                    .map(|p| matches!(&p.ty, Type::ImplTrait(_)))
                    .collect();
                sema.fn_ref_params.insert(f.name.clone(), refs);
            }
            Item::Struct(s) => {
                let fields: HashSet<String> = s.fields.iter()
                    .filter(|f| is_string_type(&f.ty))
                    .map(|f| f.name.clone())
                    .collect();
                sema.struct_string_fields.insert(s.name.clone(), fields);
            }
            _ => {}
        }
    }
    (program, sema)
}
