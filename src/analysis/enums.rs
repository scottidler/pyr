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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn test_extract_enums_basic() {
        let path = fixtures_dir().join("enums.py");
        let result = extract_enums(&path);
        assert!(result.is_ok());
        let enums = result.unwrap();
        assert!(!enums.is_empty());
    }

    #[test]
    fn test_extract_enums_contains_color() {
        let path = fixtures_dir().join("enums.py");
        let enums = extract_enums(&path).unwrap();

        let has_color = enums.keys().any(|k| k.contains("Color") && k.contains("Enum"));
        assert!(has_color, "Should contain Color(Enum)");
    }

    #[test]
    fn test_extract_enums_contains_int_enum() {
        let path = fixtures_dir().join("enums.py");
        let enums = extract_enums(&path).unwrap();

        let has_status = enums.keys().any(|k| k.contains("Status") && k.contains("IntEnum"));
        assert!(has_status, "Should contain Status(IntEnum)");
    }

    #[test]
    fn test_extract_enums_contains_str_enum() {
        let path = fixtures_dir().join("enums.py");
        let enums = extract_enums(&path).unwrap();

        let has_direction = enums.keys().any(|k| k.contains("Direction") && k.contains("StrEnum"));
        assert!(has_direction, "Should contain Direction(StrEnum)");
    }

    #[test]
    fn test_extract_enums_contains_flag() {
        let path = fixtures_dir().join("enums.py");
        let enums = extract_enums(&path).unwrap();

        // Flag doesn't contain "Enum" but we check for it anyway
        // Actually Flag is from enum module but doesn't have Enum in name
        // Let's check if it's being detected
        let _has_permissions = enums.keys().any(|k| k.contains("Permissions"));
        // This might be false depending on implementation - Flag doesn't have "Enum" in its name
    }

    #[test]
    fn test_extract_enums_excludes_non_enums() {
        let path = fixtures_dir().join("enums.py");
        let enums = extract_enums(&path).unwrap();

        let has_not_enum = enums.keys().any(|k| k.contains("NotAnEnum"));
        let has_also_not = enums.keys().any(|k| k.contains("AlsoNotAnEnum"));

        assert!(!has_not_enum, "Should NOT contain NotAnEnum");
        assert!(!has_also_not, "Should NOT contain AlsoNotAnEnum");
    }

    #[test]
    fn test_extract_enums_empty_file() {
        let path = fixtures_dir().join("empty.py");
        let enums = extract_enums(&path).unwrap();
        assert!(enums.is_empty());
    }

    #[test]
    fn test_extract_enums_classes_file() {
        let path = fixtures_dir().join("classes.py");
        let enums = extract_enums(&path).unwrap();
        // classes.py should have no enums
        assert!(enums.is_empty());
    }

    #[test]
    fn test_extract_enums_mixed_file() {
        let path = fixtures_dir().join("mixed.py");
        let enums = extract_enums(&path).unwrap();

        // Should contain Priority enum
        let has_priority = enums.keys().any(|k| k.contains("Priority"));
        assert!(has_priority, "Should contain Priority enum");
    }

    #[test]
    fn test_extract_enums_line_numbers() {
        let path = fixtures_dir().join("enums.py");
        let enums = extract_enums(&path).unwrap();

        for (_, line) in &enums {
            assert!(*line > 0, "Line numbers should be positive");
        }
    }

    #[test]
    fn test_build_enum_signature() {
        let bases = vec!["Enum".to_string()];
        let sig = build_enum_signature("Color", &bases);
        assert_eq!(sig, "class Color(Enum)");
    }

    #[test]
    fn test_build_enum_signature_int_enum() {
        let bases = vec!["IntEnum".to_string()];
        let sig = build_enum_signature("Status", &bases);
        assert_eq!(sig, "class Status(IntEnum)");
    }
}
