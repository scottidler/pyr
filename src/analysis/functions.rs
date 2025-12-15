use crate::parser::{extract_params, extract_returns, parse_file};
use eyre::Result;
use rustpython_parser::ast::{Arguments, Stmt};
use std::collections::BTreeMap;
use std::path::Path;

/// Build a function signature string
fn build_function_signature(name: &str, args: &Arguments, returns: Option<String>, is_async: bool) -> String {
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

    format!("{} {}({}){}", prefix, name, params_str.join(", "), returns_str)
}

/// Extract all top-level functions from a Python file
/// Returns a map of signature -> line number
pub fn extract_functions(path: &Path) -> Result<BTreeMap<String, usize>> {
    let parsed = parse_file(path)?;
    let mut functions = BTreeMap::new();

    for stmt in &parsed.module.body {
        if let Stmt::FunctionDef(func) = stmt {
            let name = func.name.to_string();
            let line = parsed.offset_to_line(func.range.start().into());
            let returns = extract_returns(func.returns.as_deref());
            let signature = build_function_signature(&name, &func.args, returns, false);

            functions.insert(signature, line);
        }
        // Also handle async functions
        if let Stmt::AsyncFunctionDef(func) = stmt {
            let name = func.name.to_string();
            let line = parsed.offset_to_line(func.range.start().into());
            let returns = extract_returns(func.returns.as_deref());
            let signature = build_function_signature(&name, &func.args, returns, true);

            functions.insert(signature, line);
        }
    }

    Ok(functions)
}
