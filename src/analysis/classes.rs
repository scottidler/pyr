use crate::output::ClassInfo;
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

/// Build a field signature string
fn build_field_signature(name: &str, annotation: Option<&str>) -> String {
    match annotation {
        Some(typ) => format!("{}: {}", name, typ),
        None => name.to_string(),
    }
}

/// Extract all top-level classes from a Python file (excluding enums)
/// Returns a map: class_signature -> ClassInfo (with fields and methods)
pub fn extract_classes(path: &Path) -> Result<BTreeMap<String, ClassInfo>> {
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

            // Extract fields and methods for this class
            let (fields, methods) = extract_class_members(&class.body, &parsed);

            results.insert(class_signature, ClassInfo { fields, methods });
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

/// Extract fields and methods from a class body
/// Returns (fields, methods) where each is a map of signature -> line_number
fn extract_class_members(body: &[Stmt], parsed: &ParsedFile) -> (BTreeMap<String, usize>, BTreeMap<String, usize>) {
    let mut fields = BTreeMap::new();
    let mut methods = BTreeMap::new();

    for stmt in body {
        match stmt {
            // Methods
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
            // Annotated fields: field_name: Type = value or field_name: Type
            Stmt::AnnAssign(ann) => {
                if let ast::Expr::Name(name_expr) = ann.target.as_ref() {
                    let field_name = name_expr.id.to_string();
                    let line = parsed.offset_to_line(ann.range.start().into());
                    let annotation = expr_to_string(&ann.annotation);
                    let signature = build_field_signature(&field_name, Some(&annotation));
                    fields.insert(signature, line);
                }
            }
            // Simple assignments at class level: field_name = value
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    if let ast::Expr::Name(name_expr) = target {
                        let field_name = name_expr.id.to_string();
                        // Skip dunder attributes like __slots__
                        if !field_name.starts_with("__") {
                            let line = parsed.offset_to_line(assign.range.start().into());
                            let signature = build_field_signature(&field_name, None);
                            fields.insert(signature, line);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    (fields, methods)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn test_extract_classes_simple() {
        let path = fixtures_dir().join("classes.py");
        let result = extract_classes(&path);
        assert!(result.is_ok());
        let classes = result.unwrap();
        assert!(!classes.is_empty());
    }

    #[test]
    fn test_extract_classes_excludes_enums() {
        let path = fixtures_dir().join("enums.py");
        let classes = extract_classes(&path).unwrap();
        // Enums (classes that inherit from *Enum) should be excluded
        // Note: Flag is not detected as enum because it doesn't contain "Enum" in name
        let has_color = classes.keys().any(|k| k.contains("Color"));
        let has_status = classes.keys().any(|k| k.contains("Status"));
        let has_direction = classes.keys().any(|k| k.contains("Direction"));
        assert!(!has_color, "Should not contain Color(Enum)");
        assert!(!has_status, "Should not contain Status(IntEnum)");
        assert!(!has_direction, "Should not contain Direction(StrEnum)");

        // Non-enum classes should be present
        let has_not_enum = classes.keys().any(|k| k.contains("NotAnEnum"));
        assert!(has_not_enum, "Should contain NotAnEnum");
    }

    #[test]
    fn test_extract_classes_with_bases() {
        let path = fixtures_dir().join("classes.py");
        let classes = extract_classes(&path).unwrap();

        let has_base = classes.keys().any(|k| k.contains("ClassWithBase(object)"));
        assert!(has_base, "Should contain ClassWithBase(object)");
    }

    #[test]
    fn test_extract_classes_multiple_bases() {
        let path = fixtures_dir().join("classes.py");
        let classes = extract_classes(&path).unwrap();

        let has_multi = classes.keys().any(|k| k.contains("ClassWithMultipleBases"));
        assert!(has_multi, "Should contain ClassWithMultipleBases");

        let multi = classes.keys().find(|k| k.contains("ClassWithMultipleBases")).unwrap();
        assert!(multi.contains("dict"));
        assert!(multi.contains("list"));
    }

    #[test]
    fn test_extract_classes_fields() {
        let path = fixtures_dir().join("classes.py");
        let classes = extract_classes(&path).unwrap();

        let class_info = classes
            .iter()
            .find(|(k, _)| k.contains("ClassWithFields"))
            .map(|(_, v)| v);
        assert!(class_info.is_some());

        let info = class_info.unwrap();
        assert!(!info.fields.is_empty());

        let has_name = info.fields.keys().any(|k| k.contains("name: str"));
        let has_value = info.fields.keys().any(|k| k.contains("value: int"));
        assert!(has_name, "Should have name field");
        assert!(has_value, "Should have value field");
    }

    #[test]
    fn test_extract_classes_methods() {
        let path = fixtures_dir().join("classes.py");
        let classes = extract_classes(&path).unwrap();

        let class_info = classes
            .iter()
            .find(|(k, _)| k.contains("ClassWithMethods"))
            .map(|(_, v)| v);
        assert!(class_info.is_some());

        let info = class_info.unwrap();
        assert!(!info.methods.is_empty());

        let has_public = info.methods.keys().any(|k| k.contains("public_method"));
        let has_private = info.methods.keys().any(|k| k.contains("_private_method"));
        let has_async = info.methods.keys().any(|k| k.contains("async_method"));
        assert!(has_public, "Should have public_method");
        assert!(has_private, "Should have _private_method");
        assert!(has_async, "Should have async_method");
    }

    #[test]
    fn test_extract_classes_async_methods() {
        let path = fixtures_dir().join("classes.py");
        let classes = extract_classes(&path).unwrap();

        let class_info = classes
            .iter()
            .find(|(k, _)| k.contains("ClassWithMethods"))
            .map(|(_, v)| v);
        assert!(class_info.is_some());

        let info = class_info.unwrap();
        let async_method = info.methods.keys().find(|k| k.contains("async_method"));
        assert!(async_method.is_some());
        assert!(async_method.unwrap().starts_with("async def"));
    }

    #[test]
    fn test_extract_classes_private_class() {
        let path = fixtures_dir().join("classes.py");
        let classes = extract_classes(&path).unwrap();

        let has_private = classes.keys().any(|k| k.contains("_PrivateClass"));
        assert!(has_private, "Should contain _PrivateClass");
    }

    #[test]
    fn test_extract_classes_mixed_file() {
        let path = fixtures_dir().join("mixed.py");
        let classes = extract_classes(&path).unwrap();

        // Should contain DataProcessor and ComplexTypes but NOT Priority (enum)
        let has_processor = classes.keys().any(|k| k.contains("DataProcessor"));
        let has_complex = classes.keys().any(|k| k.contains("ComplexTypes"));
        let has_priority = classes.keys().any(|k| k.contains("Priority"));

        assert!(has_processor, "Should contain DataProcessor");
        assert!(has_complex, "Should contain ComplexTypes");
        assert!(!has_priority, "Should NOT contain Priority (it's an enum)");
    }

    #[test]
    fn test_extract_classes_empty_file() {
        let path = fixtures_dir().join("empty.py");
        let classes = extract_classes(&path).unwrap();
        assert!(classes.is_empty());
    }

    #[test]
    fn test_build_class_signature_no_bases() {
        let sig = build_class_signature("MyClass", &[]);
        assert_eq!(sig, "class MyClass");
    }

    #[test]
    fn test_build_class_signature_with_bases() {
        let bases = vec!["Base".to_string(), "Mixin".to_string()];
        let sig = build_class_signature("MyClass", &bases);
        assert_eq!(sig, "class MyClass(Base, Mixin)");
    }

    #[test]
    fn test_build_field_signature_typed() {
        let sig = build_field_signature("name", Some("str"));
        assert_eq!(sig, "name: str");
    }

    #[test]
    fn test_build_field_signature_untyped() {
        let sig = build_field_signature("value", None);
        assert_eq!(sig, "value");
    }

    #[test]
    fn test_build_method_signature_sync() {
        let args = ast::Arguments {
            args: vec![],
            posonlyargs: vec![],
            vararg: None,
            kwonlyargs: vec![],
            kwarg: None,
            range: Default::default(),
        };

        let sig = build_method_signature("test", &args, Some("int".to_string()), false);
        assert_eq!(sig, "def test() -> int");
    }

    #[test]
    fn test_build_method_signature_async() {
        let args = ast::Arguments {
            args: vec![],
            posonlyargs: vec![],
            vararg: None,
            kwonlyargs: vec![],
            kwarg: None,
            range: Default::default(),
        };

        let sig = build_method_signature("test", &args, None, true);
        assert_eq!(sig, "async def test()");
    }
}
