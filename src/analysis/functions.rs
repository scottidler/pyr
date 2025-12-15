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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn test_extract_functions_simple() {
        let path = fixtures_dir().join("functions.py");
        let result = extract_functions(&path);
        assert!(result.is_ok());
        let functions = result.unwrap();
        assert!(!functions.is_empty());
    }

    #[test]
    fn test_extract_functions_contains_simple_function() {
        let path = fixtures_dir().join("functions.py");
        let functions = extract_functions(&path).unwrap();

        let has_simple = functions.keys().any(|k| k.contains("simple_function"));
        assert!(has_simple, "Should contain simple_function");
    }

    #[test]
    fn test_extract_functions_async() {
        let path = fixtures_dir().join("functions.py");
        let functions = extract_functions(&path).unwrap();

        let has_async = functions.keys().any(|k| k.starts_with("async def"));
        assert!(has_async, "Should contain async functions");
    }

    #[test]
    fn test_extract_functions_with_types() {
        let path = fixtures_dir().join("functions.py");
        let functions = extract_functions(&path).unwrap();

        let typed = functions.keys().find(|k| k.contains("function_with_types"));
        assert!(typed.is_some());
        let sig = typed.unwrap();
        assert!(sig.contains("x: int"));
        assert!(sig.contains("y: str"));
        assert!(sig.contains("-> bool"));
    }

    #[test]
    fn test_extract_functions_with_varargs() {
        let path = fixtures_dir().join("functions.py");
        let functions = extract_functions(&path).unwrap();

        let varargs = functions.keys().find(|k| k.contains("function_with_varargs"));
        assert!(varargs.is_some());
        let sig = varargs.unwrap();
        assert!(sig.contains("*args"));
        assert!(sig.contains("**kwargs"));
    }

    #[test]
    fn test_extract_functions_line_numbers() {
        let path = fixtures_dir().join("functions.py");
        let functions = extract_functions(&path).unwrap();

        // All line numbers should be positive
        for (_, line) in &functions {
            assert!(*line > 0);
        }
    }

    #[test]
    fn test_extract_functions_empty_file() {
        let path = fixtures_dir().join("empty.py");
        let functions = extract_functions(&path).unwrap();
        assert!(functions.is_empty());
    }

    #[test]
    fn test_extract_functions_mixed_file() {
        let path = fixtures_dir().join("mixed.py");
        let functions = extract_functions(&path).unwrap();

        // Should contain top-level functions but not methods
        let has_helper = functions.keys().any(|k| k.contains("helper_function"));
        let has_fetch = functions.keys().any(|k| k.contains("fetch_data"));
        let has_compute = functions.keys().any(|k| k.contains("compute_result"));

        assert!(has_helper, "Should contain helper_function");
        assert!(has_fetch, "Should contain fetch_data");
        assert!(has_compute, "Should contain compute_result");

        // Should NOT contain methods (they belong to classes)
        // 'process' is a method inside DataProcessor class, not a top-level function
        let has_process_method = functions.keys().any(|k| k == "def process(self) -> List[int]");
        assert!(
            !has_process_method,
            "Should NOT contain class methods as top-level functions"
        );
    }

    #[test]
    fn test_extract_functions_private() {
        let path = fixtures_dir().join("functions.py");
        let functions = extract_functions(&path).unwrap();

        let has_private = functions.keys().any(|k| k.contains("_private_function"));
        assert!(has_private, "Should contain _private_function");
    }

    #[test]
    fn test_build_function_signature_sync() {
        let args = Arguments {
            args: vec![],
            posonlyargs: vec![],
            vararg: None,
            kwonlyargs: vec![],
            kwarg: None,
            range: Default::default(),
        };

        let sig = build_function_signature("test", &args, Some("int".to_string()), false);
        assert_eq!(sig, "def test() -> int");
    }

    #[test]
    fn test_build_function_signature_async() {
        let args = Arguments {
            args: vec![],
            posonlyargs: vec![],
            vararg: None,
            kwonlyargs: vec![],
            kwarg: None,
            range: Default::default(),
        };

        let sig = build_function_signature("test", &args, None, true);
        assert_eq!(sig, "async def test()");
    }
}
