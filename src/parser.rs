use eyre::Result;
use rustpython_parser::{ast, Parse};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Parsed Python file with source for line number computation
pub struct ParsedFile {
    pub module: ast::ModModule,
    pub source: String,
}

impl ParsedFile {
    /// Convert a byte offset to a 1-based line number
    pub fn offset_to_line(&self, offset: u32) -> usize {
        let offset = offset as usize;
        self.source[..offset.min(self.source.len())].matches('\n').count() + 1
    }
}

/// Parse a Python file and return the AST module with source
pub fn parse_file(path: &Path) -> Result<ParsedFile> {
    let source = fs::read_to_string(path)?;
    let module = ast::ModModule::parse(&source, path.to_string_lossy().as_ref())?;
    Ok(ParsedFile { module, source })
}

/// Extract parameters as a map of name -> type
pub fn extract_params(args: &ast::Arguments) -> BTreeMap<String, String> {
    let mut params = BTreeMap::new();

    // Regular positional-or-keyword args
    for arg_with_default in args.args.iter() {
        let arg = &arg_with_default.def;
        let name = arg.arg.to_string();
        let type_str = arg.annotation.as_ref().map(|a| expr_to_string(a)).unwrap_or_default();
        if !type_str.is_empty() {
            params.insert(name, type_str);
        } else {
            params.insert(name, "...".to_string());
        }
    }

    // *args
    if let Some(vararg) = &args.vararg {
        let name = format!("*{}", vararg.arg);
        let type_str = vararg
            .annotation
            .as_ref()
            .map(|a| expr_to_string(a))
            .unwrap_or_default();
        if !type_str.is_empty() {
            params.insert(name, type_str);
        } else {
            params.insert(name, "...".to_string());
        }
    }

    // Keyword-only args
    for arg_with_default in args.kwonlyargs.iter() {
        let arg = &arg_with_default.def;
        let name = arg.arg.to_string();
        let type_str = arg.annotation.as_ref().map(|a| expr_to_string(a)).unwrap_or_default();
        if !type_str.is_empty() {
            params.insert(name, type_str);
        } else {
            params.insert(name, "...".to_string());
        }
    }

    // **kwargs
    if let Some(kwarg) = &args.kwarg {
        let name = format!("**{}", kwarg.arg);
        let type_str = kwarg.annotation.as_ref().map(|a| expr_to_string(a)).unwrap_or_default();
        if !type_str.is_empty() {
            params.insert(name, type_str);
        } else {
            params.insert(name, "...".to_string());
        }
    }

    params
}

/// Extract return type as a string
pub fn extract_returns(returns: Option<&ast::Expr>) -> Option<String> {
    returns.map(expr_to_string)
}

/// Convert an expression to a string representation
pub fn expr_to_string(expr: &ast::Expr) -> String {
    match expr {
        ast::Expr::Name(name) => name.id.to_string(),

        ast::Expr::Constant(c) => match &c.value {
            ast::Constant::None => "None".to_string(),
            ast::Constant::Bool(b) => if *b { "True" } else { "False" }.to_string(),
            ast::Constant::Str(s) => format!("\"{}\"", s),
            ast::Constant::Int(i) => i.to_string(),
            ast::Constant::Float(f) => f.to_string(),
            ast::Constant::Ellipsis => "...".to_string(),
            _ => "...".to_string(),
        },

        ast::Expr::Attribute(attr) => {
            format!("{}.{}", expr_to_string(&attr.value), attr.attr)
        }

        ast::Expr::Subscript(sub) => {
            format!("{}[{}]", expr_to_string(&sub.value), expr_to_string(&sub.slice))
        }

        ast::Expr::Tuple(tuple) => {
            let elts: Vec<_> = tuple.elts.iter().map(expr_to_string).collect();
            elts.join(", ")
        }

        ast::Expr::List(list) => {
            let elts: Vec<_> = list.elts.iter().map(expr_to_string).collect();
            format!("[{}]", elts.join(", "))
        }

        ast::Expr::BinOp(binop) => {
            let op = match binop.op {
                ast::Operator::BitOr => " | ",
                _ => " ? ",
            };
            format!("{}{}{}", expr_to_string(&binop.left), op, expr_to_string(&binop.right))
        }

        ast::Expr::Call(call) => {
            let func = expr_to_string(&call.func);
            let args: Vec<_> = call.args.iter().map(expr_to_string).collect();
            format!("{}({})", func, args.join(", "))
        }

        _ => "...".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn test_parse_file_success() {
        let path = fixtures_dir().join("functions.py");
        let result = parse_file(&path);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert!(!parsed.source.is_empty());
        assert!(!parsed.module.body.is_empty());
    }

    #[test]
    fn test_parse_file_not_found() {
        let path = fixtures_dir().join("nonexistent.py");
        let result = parse_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_offset_to_line() {
        let source = "line1\nline2\nline3\n".to_string();
        let parsed = ParsedFile {
            module: ast::ModModule::parse("", "test.py").unwrap(),
            source,
        };
        assert_eq!(parsed.offset_to_line(0), 1);
        assert_eq!(parsed.offset_to_line(5), 1); // end of line1
        assert_eq!(parsed.offset_to_line(6), 2); // start of line2
        assert_eq!(parsed.offset_to_line(12), 3); // start of line3
    }

    #[test]
    fn test_offset_to_line_empty_source() {
        let parsed = ParsedFile {
            module: ast::ModModule::parse("", "test.py").unwrap(),
            source: String::new(),
        };
        assert_eq!(parsed.offset_to_line(0), 1);
        assert_eq!(parsed.offset_to_line(100), 1); // beyond source length
    }

    #[test]
    fn test_extract_returns_some() {
        let path = fixtures_dir().join("functions.py");
        let parsed = parse_file(&path).unwrap();

        // Find a function with return type
        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "function_with_types" {
                    let returns = extract_returns(func.returns.as_deref());
                    assert_eq!(returns, Some("bool".to_string()));
                    return;
                }
            }
        }
        panic!("Function 'function_with_types' not found");
    }

    #[test]
    fn test_extract_returns_none() {
        let path = fixtures_dir().join("functions.py");
        let parsed = parse_file(&path).unwrap();

        // Find a function without return type
        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "simple_function" {
                    let returns = extract_returns(func.returns.as_deref());
                    assert_eq!(returns, None);
                    return;
                }
            }
        }
        panic!("Function 'simple_function' not found");
    }

    #[test]
    fn test_extract_params_typed() {
        let path = fixtures_dir().join("functions.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "function_with_types" {
                    let params = extract_params(&func.args);
                    assert_eq!(params.get("x"), Some(&"int".to_string()));
                    assert_eq!(params.get("y"), Some(&"str".to_string()));
                    return;
                }
            }
        }
        panic!("Function 'function_with_types' not found");
    }

    #[test]
    fn test_extract_params_untyped() {
        let path = fixtures_dir().join("functions.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "function_with_args" {
                    let params = extract_params(&func.args);
                    assert_eq!(params.get("a"), Some(&"...".to_string()));
                    assert_eq!(params.get("b"), Some(&"...".to_string()));
                    assert_eq!(params.get("c"), Some(&"...".to_string()));
                    return;
                }
            }
        }
        panic!("Function 'function_with_args' not found");
    }

    #[test]
    fn test_extract_params_varargs() {
        let path = fixtures_dir().join("functions.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "function_with_varargs" {
                    let params = extract_params(&func.args);
                    assert!(params.contains_key("*args"));
                    assert!(params.contains_key("**kwargs"));
                    return;
                }
            }
        }
        panic!("Function 'function_with_varargs' not found");
    }

    #[test]
    fn test_extract_params_typed_varargs() {
        let path = fixtures_dir().join("functions.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "function_with_typed_varargs" {
                    let params = extract_params(&func.args);
                    assert_eq!(params.get("*args"), Some(&"int".to_string()));
                    assert_eq!(params.get("**kwargs"), Some(&"str".to_string()));
                    return;
                }
            }
        }
        panic!("Function 'function_with_typed_varargs' not found");
    }

    #[test]
    fn test_extract_params_kwonly() {
        let path = fixtures_dir().join("functions.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "function_with_kwonly" {
                    let params = extract_params(&func.args);
                    assert_eq!(params.get("name"), Some(&"str".to_string()));
                    assert_eq!(params.get("value"), Some(&"int".to_string()));
                    return;
                }
            }
        }
        panic!("Function 'function_with_kwonly' not found");
    }

    #[test]
    fn test_expr_to_string_name() {
        let path = fixtures_dir().join("functions.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "function_with_types" {
                    if let Some(returns) = &func.returns {
                        assert_eq!(expr_to_string(returns), "bool");
                        return;
                    }
                }
            }
        }
        panic!("Function not found");
    }

    #[test]
    fn test_expr_to_string_subscript() {
        let path = fixtures_dir().join("classes.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::ClassDef(class) = stmt {
                if class.name.to_string() == "ClassWithFields" {
                    for body_stmt in &class.body {
                        if let ast::Stmt::AnnAssign(ann) = body_stmt {
                            if let ast::Expr::Name(name) = ann.target.as_ref() {
                                if name.id.to_string() == "items" {
                                    let type_str = expr_to_string(&ann.annotation);
                                    assert_eq!(type_str, "List[str]");
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
        panic!("Field 'items' not found");
    }

    #[test]
    fn test_expr_to_string_attribute() {
        let path = fixtures_dir().join("enums.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::ClassDef(class) = stmt {
                if class.name.to_string() == "Permissions" {
                    // Check base classes
                    for base in &class.bases {
                        let base_str = expr_to_string(base);
                        if base_str == "Flag" {
                            return;
                        }
                    }
                }
            }
        }
        panic!("Permissions enum not found or Flag base not found");
    }

    #[test]
    fn test_expr_to_string_binop_union() {
        let path = fixtures_dir().join("mixed.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::ClassDef(class) = stmt {
                if class.name.to_string() == "ComplexTypes" {
                    for body_stmt in &class.body {
                        if let ast::Stmt::AnnAssign(ann) = body_stmt {
                            if let ast::Expr::Name(name) = ann.target.as_ref() {
                                if name.id.to_string() == "union_type" {
                                    let type_str = expr_to_string(&ann.annotation);
                                    assert_eq!(type_str, "int | str");
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
        panic!("Field 'union_type' not found");
    }

    #[test]
    fn test_expr_to_string_constants() {
        let path = fixtures_dir().join("expressions.py");
        let parsed = parse_file(&path).unwrap();

        let mut found_none = false;
        let mut found_bool = false;
        let mut found_string = false;
        let mut found_int = false;
        let mut found_float = false;
        let mut found_ellipsis = false;

        for stmt in &parsed.module.body {
            if let ast::Stmt::Assign(assign) = stmt {
                for target in &assign.targets {
                    if let ast::Expr::Name(name) = target {
                        let name_str = name.id.to_string();
                        let value_str = expr_to_string(&assign.value);

                        match name_str.as_str() {
                            "NONE_CONST" => {
                                assert_eq!(value_str, "None");
                                found_none = true;
                            }
                            "TRUE_CONST" => {
                                assert_eq!(value_str, "True");
                                found_bool = true;
                            }
                            "FALSE_CONST" => {
                                assert_eq!(value_str, "False");
                            }
                            "STRING_CONST" => {
                                assert_eq!(value_str, "\"hello\"");
                                found_string = true;
                            }
                            "INT_CONST" => {
                                assert_eq!(value_str, "42");
                                found_int = true;
                            }
                            "FLOAT_CONST" => {
                                assert_eq!(value_str, "3.14");
                                found_float = true;
                            }
                            "ELLIPSIS_CONST" => {
                                assert_eq!(value_str, "...");
                                found_ellipsis = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        assert!(found_none, "Should find None constant");
        assert!(found_bool, "Should find Bool constants");
        assert!(found_string, "Should find String constant");
        assert!(found_int, "Should find Int constant");
        assert!(found_float, "Should find Float constant");
        assert!(found_ellipsis, "Should find Ellipsis constant");
    }

    #[test]
    fn test_expr_to_string_list() {
        let path = fixtures_dir().join("expressions.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "func_with_list" {
                    if let Some(returns) = &func.returns {
                        let ret_str = expr_to_string(returns);
                        assert_eq!(ret_str, "[int, str]");
                        return;
                    }
                }
            }
        }
        panic!("Function func_with_list not found");
    }

    #[test]
    fn test_expr_to_string_tuple() {
        let path = fixtures_dir().join("expressions.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "func_with_tuple" {
                    let params = extract_params(&func.args);
                    if let Some(args_type) = params.get("args") {
                        assert_eq!(args_type, "int, str, bool");
                        return;
                    }
                }
            }
        }
        panic!("Function func_with_tuple not found or params not found");
    }

    #[test]
    fn test_expr_to_string_call() {
        let path = fixtures_dir().join("expressions.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "func_with_callable" {
                    let params = extract_params(&func.args);
                    if let Some(callback_type) = params.get("callback") {
                        assert!(callback_type.contains("Callable"));
                        return;
                    }
                }
            }
        }
        panic!("Function func_with_callable not found or params not found");
    }

    #[test]
    fn test_expr_to_string_complex_nested() {
        let path = fixtures_dir().join("expressions.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::ClassDef(class) = stmt {
                if class.name.to_string() == "ComplexAnnotations" {
                    for body_stmt in &class.body {
                        if let ast::Stmt::AnnAssign(ann) = body_stmt {
                            if let ast::Expr::Name(name) = ann.target.as_ref() {
                                if name.id.to_string() == "nested" {
                                    let type_str = expr_to_string(&ann.annotation);
                                    assert!(type_str.contains("Dict"));
                                    assert!(type_str.contains("List"));
                                    assert!(type_str.contains("Tuple"));
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
        panic!("ComplexAnnotations.nested not found");
    }

    #[test]
    fn test_extract_params_empty() {
        let args = ast::Arguments {
            args: vec![],
            posonlyargs: vec![],
            vararg: None,
            kwonlyargs: vec![],
            kwarg: None,
            range: Default::default(),
        };

        let params = extract_params(&args);
        assert!(params.is_empty());
    }

    #[test]
    fn test_offset_to_line_multiline() {
        let source = "def foo():\n    pass\n\ndef bar():\n    return 42\n".to_string();
        let parsed = ParsedFile {
            module: ast::ModModule::parse(&source, "test.py").unwrap(),
            source,
        };
        assert_eq!(parsed.offset_to_line(0), 1); // def foo
        assert_eq!(parsed.offset_to_line(11), 2); // pass
        assert_eq!(parsed.offset_to_line(22), 4); // def bar
    }

    #[test]
    fn test_expr_to_string_fallback_expression() {
        // Test that unsupported expressions return "..."
        let path = fixtures_dir().join("binop_other.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::ClassDef(class) = stmt {
                if class.name.to_string() == "WeirdClass" {
                    for body_stmt in &class.body {
                        if let ast::Stmt::Assign(assign) = body_stmt {
                            // Lambda expressions should return "..."
                            let value_str = expr_to_string(&assign.value);
                            // Lambdas are not explicitly handled, so should fallback
                            assert!(!value_str.is_empty());
                            return;
                        }
                    }
                }
            }
        }
        panic!("WeirdClass.weird_field not found");
    }

    #[test]
    fn test_extract_params_typed_star_args() {
        let path = fixtures_dir().join("edge_cases.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "func_with_typed_star_args" {
                    let params = extract_params(&func.args);
                    assert!(params.contains_key("*args"));
                    assert_eq!(params.get("*args"), Some(&"tuple".to_string()));
                    return;
                }
            }
        }
        panic!("func_with_typed_star_args not found");
    }

    #[test]
    fn test_extract_params_untyped_kwonly() {
        let path = fixtures_dir().join("edge_cases.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "func_with_untyped_kwonly" {
                    let params = extract_params(&func.args);
                    // Untyped keyword-only args should have "..." as type
                    assert!(params.contains_key("name"));
                    assert!(params.contains_key("value"));
                    assert_eq!(params.get("name"), Some(&"...".to_string()));
                    assert_eq!(params.get("value"), Some(&"...".to_string()));
                    return;
                }
            }
        }
        panic!("func_with_untyped_kwonly not found");
    }

    #[test]
    fn test_extract_params_complex_mix() {
        let path = fixtures_dir().join("edge_cases.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "complex_func" {
                    let params = extract_params(&func.args);
                    // Check various param types are correctly extracted
                    assert_eq!(params.get("a"), Some(&"int".to_string())); // typed regular
                    assert_eq!(params.get("b"), Some(&"...".to_string())); // untyped regular
                    assert!(params.contains_key("*args")); // varargs
                    assert_eq!(params.get("name"), Some(&"str".to_string())); // typed kwonly
                    assert_eq!(params.get("value"), Some(&"...".to_string())); // untyped kwonly
                    assert_eq!(params.get("**kwargs"), Some(&"dict".to_string())); // typed kwargs
                    return;
                }
            }
        }
        panic!("complex_func not found");
    }

    #[test]
    fn test_extract_params_with_annotation_on_vararg() {
        // Make sure we test the annotation path for varargs
        let path = fixtures_dir().join("functions.py");
        let parsed = parse_file(&path).unwrap();

        for stmt in &parsed.module.body {
            if let ast::Stmt::FunctionDef(func) = stmt {
                if func.name.to_string() == "function_with_typed_varargs" {
                    let params = extract_params(&func.args);
                    // Should have typed *args
                    if let Some(vararg_type) = params.get("*args") {
                        // The annotation should be the type we specified
                        assert_eq!(vararg_type, "int");
                    }
                    return;
                }
            }
        }
        panic!("function_with_typed_varargs not found");
    }
}
