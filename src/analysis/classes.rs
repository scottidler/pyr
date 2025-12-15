use crate::parser::{ParsedFile, expr_to_string, extract_params, extract_returns, parse_file};
use eyre::Result;
use rustpython_parser::ast::{self, Arguments, Stmt};
use std::collections::BTreeMap;
use std::path::Path;

/// Build a method signature string (without class prefix since it's nested under class)
fn build_method_signature(method_name: &str, args: &Arguments, returns: Option<String>, is_async: bool) -> String {
    let params = extract_params(args);
    let params_str: Vec<String> = params
        .iter()
        .map(
            |(name, typ)| {
                if typ == "..." { name.clone() } else { format!("{}: {}", name, typ) }
            },
        )
        .collect();

    let prefix = if is_async { "async def" } else { "def" };
    let returns_str = returns.map(|r| format!(" -> {}", r)).unwrap_or_default();

    format!("{} {}({}){}", prefix, method_name, params_str.join(", "), returns_str)
}

/// Build a class signature string
fn build_class_signature(name: &str, bases: &[String]) -> String {
    if bases.is_empty() {
        format!("class {}", name)
    } else {
        format!("class {}({})", name, bases.join(", "))
    }
}

/// Extract all top-level classes from a Python file (excluding enums)
/// Returns a nested map: class_signature -> (method_signature -> line_number)
pub fn extract_classes(path: &Path) -> Result<BTreeMap<String, BTreeMap<String, usize>>> {
    let parsed = parse_file(path)?;
    let mut results = BTreeMap::new();

    for stmt in &parsed.module.body {
        if let Stmt::ClassDef(class) = stmt {
            // Skip if this is an enum (handled by enums module)
            if is_enum(class) {
                continue;
            }

            let name = class.name.to_string();
            let bases: Vec<String> = class.bases.iter().map(expr_to_string).collect();
            let class_signature = build_class_signature(&name, &bases);

            // Extract methods for this class
            let methods = extract_methods(&class.body, &parsed);

            results.insert(class_signature, methods);
        }
    }

    Ok(results)
}

/// Check if a class is an enum based on its base classes
fn is_enum(class: &ast::StmtClassDef) -> bool {
    class.bases.iter().any(|base| {
        let base_str = expr_to_string(base);
        base_str.contains("Enum")
    })
}

/// Extract methods from a class body
/// Returns a map of method_signature -> line_number
fn extract_methods(body: &[Stmt], parsed: &ParsedFile) -> BTreeMap<String, usize> {
    let mut methods = BTreeMap::new();

    for stmt in body {
        match stmt {
            Stmt::FunctionDef(func) => {
                let name = func.name.to_string();
                let line = parsed.offset_to_line(func.range.start().into());
                let returns = extract_returns(func.returns.as_deref());
                let signature = build_method_signature(&name, &func.args, returns, false);

                methods.insert(signature, line);
            }
            Stmt::AsyncFunctionDef(func) => {
                let name = func.name.to_string();
                let line = parsed.offset_to_line(func.range.start().into());
                let returns = extract_returns(func.returns.as_deref());
                let signature = build_method_signature(&name, &func.args, returns, true);

                methods.insert(signature, line);
            }
            _ => {}
        }
    }

    methods
}
