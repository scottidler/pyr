use crate::parser::{expr_to_string, parse_file};
use eyre::Result;
use rustpython_parser::ast::{self, Stmt};
use std::collections::BTreeMap;
use std::path::Path;

/// Build an enum signature string
fn build_enum_signature(name: &str, bases: &[String]) -> String {
    format!("class {}({})", name, bases.join(", "))
}

/// Extract all enum definitions from a Python file
/// Returns a map of signature -> line number
pub fn extract_enums(path: &Path) -> Result<BTreeMap<String, usize>> {
    let parsed = parse_file(path)?;
    let mut enums = BTreeMap::new();

    for stmt in &parsed.module.body {
        if let Stmt::ClassDef(class) = stmt {
            if !is_enum(class) {
                continue;
            }

            let name = class.name.to_string();
            let line = parsed.offset_to_line(class.range.start().into());
            let bases: Vec<String> = class.bases.iter().map(expr_to_string).collect();
            let signature = build_enum_signature(&name, &bases);

            enums.insert(signature, line);
        }
    }

    Ok(enums)
}

/// Check if a class is an enum based on its base classes
fn is_enum(class: &ast::StmtClassDef) -> bool {
    class.bases.iter().any(|base| {
        let base_str = expr_to_string(base);
        base_str.contains("Enum")
    })
}
