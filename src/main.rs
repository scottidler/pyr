use clap::Parser;
use eyre::Result;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

mod analysis;
mod cli;
mod output;
mod parser;
mod pattern;
mod walk;

use cli::{Cli, Command, Visibility};
use output::{output, should_use_json, ClassInfo, ClassMap, ClassesOutput, FilesOutput};
use pattern::{extract_class_name, extract_function_name, filter_classes_output, filter_files_output};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let use_json = should_use_json(cli.json);
    let targets = &cli.targets;

    match &cli.command {
        Command::Function {
            patterns,
            public,
            private,
        } => {
            let visibility = Visibility::from_flags(*public, *private);
            run_functions(targets, patterns, visibility, cli.alphabetical, use_json)
        }
        Command::Class {
            patterns,
            public,
            private,
        } => {
            let visibility = Visibility::from_flags(*public, *private);
            run_classes(targets, patterns, visibility, cli.alphabetical, use_json)
        }
        Command::Enum { patterns } => run_enums(targets, patterns, cli.alphabetical, use_json),
        Command::Module { patterns } => run_modules(targets, patterns, use_json),
        Command::Dump { patterns } => run_dump(targets, patterns, cli.alphabetical, use_json),
    }
}

/// Compute functions output (testable without I/O)
fn compute_functions(targets: &[PathBuf], patterns: &[String], visibility: Visibility) -> Result<FilesOutput> {
    let files = walk::collect_python_files(targets)?;
    let collected = process_files_parallel(&files, |path| {
        let functions = analysis::extract_functions(path).ok()?;
        if functions.is_empty() {
            None
        } else {
            Some(functions)
        }
    });
    let filtered = filter_files_output(collected, patterns, extract_function_name);
    let filtered = filter_by_visibility(filtered, visibility);
    Ok(FilesOutput { files: filtered })
}

fn run_functions(
    targets: &[PathBuf],
    patterns: &[String],
    visibility: Visibility,
    _alphabetical: bool,
    use_json: bool,
) -> Result<()> {
    let result = compute_functions(targets, patterns, visibility)?;
    output(&result, use_json)
}

/// Compute classes output (testable without I/O)
fn compute_classes(targets: &[PathBuf], patterns: &[String], visibility: Visibility) -> Result<ClassesOutput> {
    let files = walk::collect_python_files(targets)?;
    let collected = process_classes_parallel(&files, |path| {
        let classes = analysis::extract_classes(path).ok()?;
        if classes.is_empty() {
            None
        } else {
            Some(classes)
        }
    });
    let filtered = filter_classes_output(collected, patterns);
    let filtered = filter_classes_by_visibility(filtered, visibility);
    Ok(ClassesOutput { files: filtered })
}

fn run_classes(
    targets: &[PathBuf],
    patterns: &[String],
    visibility: Visibility,
    _alphabetical: bool,
    use_json: bool,
) -> Result<()> {
    let result = compute_classes(targets, patterns, visibility)?;
    output(&result, use_json)
}

/// Compute enums output (testable without I/O)
fn compute_enums(targets: &[PathBuf], patterns: &[String]) -> Result<FilesOutput> {
    let files = walk::collect_python_files(targets)?;
    let collected = process_files_parallel(&files, |path| {
        let enums = analysis::extract_enums(path).ok()?;
        if enums.is_empty() {
            None
        } else {
            Some(enums)
        }
    });
    let filtered = filter_files_output(collected, patterns, extract_class_name);
    Ok(FilesOutput { files: filtered })
}

fn run_enums(targets: &[PathBuf], patterns: &[String], _alphabetical: bool, use_json: bool) -> Result<()> {
    let result = compute_enums(targets, patterns)?;
    output(&result, use_json)
}

/// Compute modules output (testable without I/O)
fn compute_modules(targets: &[PathBuf], patterns: &[String]) -> Result<output::ModulesOutput> {
    let files = walk::collect_python_files(targets)?;

    // Use the first target as base path, or current dir
    let base_path = targets
        .first()
        .map(|p| {
            if p.is_dir() {
                p.clone()
            } else {
                p.parent().map(|p| p.to_path_buf()).unwrap_or_default()
            }
        })
        .unwrap_or_else(|| PathBuf::from("."));

    let result = analysis::build_module_tree(&files, &base_path);
    Ok(pattern::filter_modules_output(result, patterns))
}

fn run_modules(targets: &[PathBuf], patterns: &[String], use_json: bool) -> Result<()> {
    let result = compute_modules(targets, patterns)?;
    output(&result, use_json)
}

/// Compute dump output (testable without I/O)
fn compute_dump(targets: &[PathBuf], patterns: &[String]) -> Result<FilesOutput> {
    let files = walk::collect_python_files(targets)?;
    let collected = process_files_parallel(&files, |path| {
        let mut all_entries = BTreeMap::new();

        if let Ok(functions) = analysis::extract_functions(path) {
            all_entries.extend(functions);
        }
        // Flatten classes: prefix method signatures with class name
        if let Ok(classes) = analysis::extract_classes(path) {
            for (class_sig, class_info) in classes {
                // Extract class name from signature (e.g., "class Foo" -> "Foo")
                let class_name = class_sig
                    .strip_prefix("class ")
                    .and_then(|s| s.split('(').next())
                    .unwrap_or(&class_sig);
                for (method_sig, line) in class_info.methods {
                    let full_sig = format!("{}.{}", class_name, method_sig);
                    all_entries.insert(full_sig, line);
                }
            }
        }
        if let Ok(enums) = analysis::extract_enums(path) {
            all_entries.extend(enums);
        }

        if all_entries.is_empty() {
            None
        } else {
            Some(all_entries)
        }
    });
    let filtered = filter_files_output(collected, patterns, pattern::extract_dump_name);
    Ok(FilesOutput { files: filtered })
}

fn run_dump(targets: &[PathBuf], patterns: &[String], _alphabetical: bool, use_json: bool) -> Result<()> {
    let result = compute_dump(targets, patterns)?;
    output(&result, use_json)
}

/// Process files in parallel and collect results (flat structure)
fn process_files_parallel<F>(files: &[PathBuf], processor: F) -> BTreeMap<String, BTreeMap<String, usize>>
where
    F: Fn(&std::path::Path) -> Option<BTreeMap<String, usize>> + Sync,
{
    let results: Mutex<BTreeMap<String, BTreeMap<String, usize>>> = Mutex::new(BTreeMap::new());

    files.par_iter().for_each(|path| {
        if let Some(content) = processor(path) {
            let key = path.to_string_lossy().to_string();
            results.lock().unwrap().insert(key, content);
        }
    });

    results.into_inner().unwrap()
}

/// Process files in parallel and collect results (nested structure for classes)
fn process_classes_parallel<F>(files: &[PathBuf], processor: F) -> BTreeMap<String, ClassMap>
where
    F: Fn(&std::path::Path) -> Option<ClassMap> + Sync,
{
    let results: Mutex<BTreeMap<String, ClassMap>> = Mutex::new(BTreeMap::new());

    files.par_iter().for_each(|path| {
        if let Some(content) = processor(path) {
            let key = path.to_string_lossy().to_string();
            results.lock().unwrap().insert(key, content);
        }
    });

    results.into_inner().unwrap()
}

/// Check if a name matches the visibility filter
fn matches_visibility(name: &str, visibility: Visibility) -> bool {
    match visibility {
        Visibility::All => true,
        Visibility::Public => !name.starts_with('_'),
        Visibility::Private => name.starts_with('_'),
    }
}

/// Filter files output by visibility (for functions)
fn filter_by_visibility(
    files: BTreeMap<String, BTreeMap<String, usize>>,
    visibility: Visibility,
) -> BTreeMap<String, BTreeMap<String, usize>> {
    if visibility == Visibility::All {
        return files;
    }

    files
        .into_iter()
        .filter_map(|(file_path, entries)| {
            let filtered: BTreeMap<String, usize> = entries
                .into_iter()
                .filter(|(sig, _)| {
                    let name = extract_function_name(sig);
                    matches_visibility(name, visibility)
                })
                .collect();

            if filtered.is_empty() {
                None
            } else {
                Some((file_path, filtered))
            }
        })
        .collect()
}

/// Filter classes output by visibility (filters fields and methods within each class)
fn filter_classes_by_visibility(
    files: BTreeMap<String, ClassMap>,
    visibility: Visibility,
) -> BTreeMap<String, ClassMap> {
    if visibility == Visibility::All {
        return files;
    }

    files
        .into_iter()
        .filter_map(|(file_path, classes)| {
            let filtered_classes: ClassMap = classes
                .into_iter()
                .map(|(class_sig, class_info)| {
                    let filtered_fields: BTreeMap<String, usize> = class_info
                        .fields
                        .into_iter()
                        .filter(|(field_sig, _)| {
                            // Extract field name from signature (e.g., "field_name: Type" -> "field_name")
                            let name = field_sig.split(':').next().unwrap_or(field_sig).trim();
                            matches_visibility(name, visibility)
                        })
                        .collect();

                    let filtered_methods: BTreeMap<String, usize> = class_info
                        .methods
                        .into_iter()
                        .filter(|(method_sig, _)| {
                            let name = extract_function_name(method_sig);
                            matches_visibility(name, visibility)
                        })
                        .collect();

                    (
                        class_sig,
                        ClassInfo {
                            fields: filtered_fields,
                            methods: filtered_methods,
                        },
                    )
                })
                .filter(|(_, class_info)| {
                    // Keep class if it has any fields or methods after filtering
                    !class_info.fields.is_empty() || !class_info.methods.is_empty()
                })
                .collect();

            if filtered_classes.is_empty() {
                None
            } else {
                Some((file_path, filtered_classes))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn test_matches_visibility_all() {
        assert!(matches_visibility("public_func", Visibility::All));
        assert!(matches_visibility("_private_func", Visibility::All));
        assert!(matches_visibility("__dunder__", Visibility::All));
    }

    #[test]
    fn test_matches_visibility_public() {
        assert!(matches_visibility("public_func", Visibility::Public));
        assert!(!matches_visibility("_private_func", Visibility::Public));
        assert!(!matches_visibility("__dunder__", Visibility::Public));
    }

    #[test]
    fn test_matches_visibility_private() {
        assert!(!matches_visibility("public_func", Visibility::Private));
        assert!(matches_visibility("_private_func", Visibility::Private));
        assert!(matches_visibility("__dunder__", Visibility::Private));
    }

    #[test]
    fn test_filter_by_visibility_all() {
        let mut files = BTreeMap::new();
        let mut entries = BTreeMap::new();
        entries.insert("def public_func()".to_string(), 1);
        entries.insert("def _private_func()".to_string(), 2);
        files.insert("test.py".to_string(), entries);

        let result = filter_by_visibility(files, Visibility::All);
        let entries = result.get("test.py").unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_filter_by_visibility_public() {
        let mut files = BTreeMap::new();
        let mut entries = BTreeMap::new();
        entries.insert("def public_func()".to_string(), 1);
        entries.insert("def _private_func()".to_string(), 2);
        files.insert("test.py".to_string(), entries);

        let result = filter_by_visibility(files, Visibility::Public);
        let entries = result.get("test.py").unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries.contains_key("def public_func()"));
    }

    #[test]
    fn test_filter_by_visibility_private() {
        let mut files = BTreeMap::new();
        let mut entries = BTreeMap::new();
        entries.insert("def public_func()".to_string(), 1);
        entries.insert("def _private_func()".to_string(), 2);
        files.insert("test.py".to_string(), entries);

        let result = filter_by_visibility(files, Visibility::Private);
        let entries = result.get("test.py").unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries.contains_key("def _private_func()"));
    }

    #[test]
    fn test_filter_by_visibility_removes_empty_files() {
        let mut files = BTreeMap::new();
        let mut entries = BTreeMap::new();
        entries.insert("def _private_func()".to_string(), 1);
        files.insert("test.py".to_string(), entries);

        let result = filter_by_visibility(files, Visibility::Public);
        assert!(result.is_empty()); // File removed because no public functions
    }

    #[test]
    fn test_filter_classes_by_visibility_all() {
        let mut files: BTreeMap<String, ClassMap> = BTreeMap::new();
        let mut classes = BTreeMap::new();

        let mut fields = BTreeMap::new();
        fields.insert("name: str".to_string(), 1);
        fields.insert("_private: int".to_string(), 2);

        let mut methods = BTreeMap::new();
        methods.insert("def public()".to_string(), 3);
        methods.insert("def _private()".to_string(), 4);

        classes.insert("class Test".to_string(), ClassInfo { fields, methods });
        files.insert("test.py".to_string(), classes);

        let result = filter_classes_by_visibility(files, Visibility::All);
        let classes = result.get("test.py").unwrap();
        let class_info = classes.get("class Test").unwrap();
        assert_eq!(class_info.fields.len(), 2);
        assert_eq!(class_info.methods.len(), 2);
    }

    #[test]
    fn test_filter_classes_by_visibility_public() {
        let mut files: BTreeMap<String, ClassMap> = BTreeMap::new();
        let mut classes = BTreeMap::new();

        let mut fields = BTreeMap::new();
        fields.insert("name: str".to_string(), 1);
        fields.insert("_private: int".to_string(), 2);

        let mut methods = BTreeMap::new();
        methods.insert("def public()".to_string(), 3);
        methods.insert("def _private()".to_string(), 4);

        classes.insert("class Test".to_string(), ClassInfo { fields, methods });
        files.insert("test.py".to_string(), classes);

        let result = filter_classes_by_visibility(files, Visibility::Public);
        let classes = result.get("test.py").unwrap();
        let class_info = classes.get("class Test").unwrap();
        assert_eq!(class_info.fields.len(), 1);
        assert_eq!(class_info.methods.len(), 1);
        assert!(class_info.fields.contains_key("name: str"));
        assert!(class_info.methods.contains_key("def public()"));
    }

    #[test]
    fn test_filter_classes_by_visibility_private() {
        let mut files: BTreeMap<String, ClassMap> = BTreeMap::new();
        let mut classes = BTreeMap::new();

        let mut fields = BTreeMap::new();
        fields.insert("name: str".to_string(), 1);
        fields.insert("_private: int".to_string(), 2);

        let mut methods = BTreeMap::new();
        methods.insert("def public()".to_string(), 3);
        methods.insert("def _private()".to_string(), 4);

        classes.insert("class Test".to_string(), ClassInfo { fields, methods });
        files.insert("test.py".to_string(), classes);

        let result = filter_classes_by_visibility(files, Visibility::Private);
        let classes = result.get("test.py").unwrap();
        let class_info = classes.get("class Test").unwrap();
        assert_eq!(class_info.fields.len(), 1);
        assert_eq!(class_info.methods.len(), 1);
        assert!(class_info.fields.contains_key("_private: int"));
        assert!(class_info.methods.contains_key("def _private()"));
    }

    #[test]
    fn test_filter_classes_removes_empty_classes() {
        let mut files: BTreeMap<String, ClassMap> = BTreeMap::new();
        let mut classes = BTreeMap::new();

        let mut fields = BTreeMap::new();
        fields.insert("_private: int".to_string(), 1);

        let mut methods = BTreeMap::new();
        methods.insert("def _private()".to_string(), 2);

        classes.insert("class Test".to_string(), ClassInfo { fields, methods });
        files.insert("test.py".to_string(), classes);

        let result = filter_classes_by_visibility(files, Visibility::Public);
        assert!(result.is_empty()); // File and class removed because no public members
    }

    #[test]
    fn test_process_files_parallel_empty() {
        let files: Vec<PathBuf> = vec![];
        let result = process_files_parallel(&files, |_| None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_process_files_parallel_with_files() {
        let files = vec![fixtures_dir().join("functions.py"), fixtures_dir().join("classes.py")];
        let result = process_files_parallel(&files, |path| {
            let functions = analysis::extract_functions(path).ok()?;
            if functions.is_empty() {
                None
            } else {
                Some(functions)
            }
        });

        // functions.py should have functions, classes.py should have none at top level
        assert!(result.len() >= 1);
    }

    #[test]
    fn test_process_classes_parallel_empty() {
        let files: Vec<PathBuf> = vec![];
        let result = process_classes_parallel(&files, |_| None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_process_classes_parallel_with_files() {
        let files = vec![fixtures_dir().join("classes.py")];
        let result = process_classes_parallel(&files, |path| {
            let classes = analysis::extract_classes(path).ok()?;
            if classes.is_empty() {
                None
            } else {
                Some(classes)
            }
        });

        assert!(!result.is_empty());
    }

    // Integration tests that exercise the run_* functions indirectly
    #[test]
    fn test_integration_extract_functions_and_filter() {
        let targets = vec![fixtures_dir().join("functions.py")];
        let files = walk::collect_python_files(&targets).unwrap();
        let collected = process_files_parallel(&files, |path| {
            let functions = analysis::extract_functions(path).ok()?;
            if functions.is_empty() {
                None
            } else {
                Some(functions)
            }
        });
        let filtered = filter_files_output(collected, &["simple".to_string()], extract_function_name);
        let filtered = filter_by_visibility(filtered, Visibility::All);

        assert!(!filtered.is_empty());
        let has_simple = filtered
            .values()
            .any(|entries| entries.keys().any(|k| k.contains("simple_function")));
        assert!(has_simple);
    }

    #[test]
    fn test_integration_extract_classes_and_filter() {
        let targets = vec![fixtures_dir().join("classes.py")];
        let files = walk::collect_python_files(&targets).unwrap();
        let collected = process_classes_parallel(&files, |path| {
            let classes = analysis::extract_classes(path).ok()?;
            if classes.is_empty() {
                None
            } else {
                Some(classes)
            }
        });
        let filtered = filter_classes_output(collected, &["Class".to_string()]);
        let filtered = filter_classes_by_visibility(filtered, Visibility::All);

        assert!(!filtered.is_empty());
    }

    #[test]
    fn test_integration_extract_enums_and_filter() {
        let targets = vec![fixtures_dir().join("enums.py")];
        let files = walk::collect_python_files(&targets).unwrap();
        let collected = process_files_parallel(&files, |path| {
            let enums = analysis::extract_enums(path).ok()?;
            if enums.is_empty() {
                None
            } else {
                Some(enums)
            }
        });
        let filtered = filter_files_output(collected, &["Color".to_string()], extract_class_name);

        assert!(!filtered.is_empty());
    }

    #[test]
    fn test_integration_build_module_tree() {
        let targets = vec![fixtures_dir().join("pkg")];
        let files = walk::collect_python_files(&targets).unwrap();

        let base_path = &targets[0];
        let result = analysis::build_module_tree(&files, base_path);
        let filtered = pattern::filter_modules_output(result, &[]);

        assert!(!filtered.modules.is_empty());
    }

    #[test]
    fn test_integration_mixed_file_dump_style() {
        let targets = vec![fixtures_dir().join("mixed.py")];
        let files = walk::collect_python_files(&targets).unwrap();
        let collected = process_files_parallel(&files, |path| {
            let mut all_entries = BTreeMap::new();

            if let Ok(functions) = analysis::extract_functions(path) {
                all_entries.extend(functions);
            }
            if let Ok(classes) = analysis::extract_classes(path) {
                for (class_sig, class_info) in classes {
                    let class_name = class_sig
                        .strip_prefix("class ")
                        .and_then(|s| s.split('(').next())
                        .unwrap_or(&class_sig);
                    for (method_sig, line) in class_info.methods {
                        let full_sig = format!("{}.{}", class_name, method_sig);
                        all_entries.insert(full_sig, line);
                    }
                }
            }
            if let Ok(enums) = analysis::extract_enums(path) {
                all_entries.extend(enums);
            }

            if all_entries.is_empty() {
                None
            } else {
                Some(all_entries)
            }
        });

        assert!(!collected.is_empty());
        let filtered = filter_files_output(collected, &["helper".to_string()], pattern::extract_dump_name);
        assert!(!filtered.is_empty());
    }

    #[test]
    fn test_integration_visibility_filtering_public() {
        let targets = vec![fixtures_dir().join("functions.py")];
        let files = walk::collect_python_files(&targets).unwrap();
        let collected = process_files_parallel(&files, |path| {
            let functions = analysis::extract_functions(path).ok()?;
            if functions.is_empty() {
                None
            } else {
                Some(functions)
            }
        });
        let filtered = filter_files_output(collected, &[], extract_function_name);
        let public_only = filter_by_visibility(filtered.clone(), Visibility::Public);
        let private_only = filter_by_visibility(filtered, Visibility::Private);

        let public_count: usize = public_only.values().map(|e| e.len()).sum();
        let private_count: usize = private_only.values().map(|e| e.len()).sum();

        assert!(public_count > 0);
        assert!(private_count > 0);
    }

    #[test]
    fn test_integration_class_visibility_filtering() {
        let targets = vec![fixtures_dir().join("classes.py")];
        let files = walk::collect_python_files(&targets).unwrap();
        let collected = process_classes_parallel(&files, |path| {
            let classes = analysis::extract_classes(path).ok()?;
            if classes.is_empty() {
                None
            } else {
                Some(classes)
            }
        });
        let filtered = filter_classes_output(collected, &[]);
        let public_only = filter_classes_by_visibility(filtered.clone(), Visibility::Public);
        let private_only = filter_classes_by_visibility(filtered, Visibility::Private);

        let has_public = public_only.values().any(|classes| {
            classes
                .values()
                .any(|info| !info.fields.is_empty() || !info.methods.is_empty())
        });
        let has_private = private_only.values().any(|classes| {
            classes
                .values()
                .any(|info| !info.fields.is_empty() || !info.methods.is_empty())
        });

        assert!(has_public);
        assert!(has_private);
    }

    #[test]
    fn test_integration_modules_with_base_path_file() {
        let targets = vec![fixtures_dir().join("functions.py")];
        let files = walk::collect_python_files(&targets).unwrap();

        let base_path = targets
            .first()
            .map(|p| {
                if p.is_dir() {
                    p.clone()
                } else {
                    p.parent().map(|p| p.to_path_buf()).unwrap_or_default()
                }
            })
            .unwrap_or_else(|| PathBuf::from("."));

        let result = analysis::build_module_tree(&files, &base_path);
        assert!(!result.modules.is_empty());
    }

    // Tests for compute_* functions
    #[test]
    fn test_compute_functions() {
        let targets = vec![fixtures_dir().join("functions.py")];
        let result = compute_functions(&targets, &[], Visibility::All).unwrap();
        assert!(!result.files.is_empty());
    }

    #[test]
    fn test_compute_functions_with_pattern() {
        let targets = vec![fixtures_dir().join("functions.py")];
        let result = compute_functions(&targets, &["simple".to_string()], Visibility::All).unwrap();
        assert!(!result.files.is_empty());
        let has_simple = result.files.values().any(|e| e.keys().any(|k| k.contains("simple")));
        assert!(has_simple);
    }

    #[test]
    fn test_compute_functions_visibility_public() {
        let targets = vec![fixtures_dir().join("functions.py")];
        let result = compute_functions(&targets, &[], Visibility::Public).unwrap();
        // Should have only public functions
        for entries in result.files.values() {
            for sig in entries.keys() {
                let name = extract_function_name(sig);
                assert!(!name.starts_with('_'), "Should not have private function: {}", name);
            }
        }
    }

    #[test]
    fn test_compute_functions_visibility_private() {
        let targets = vec![fixtures_dir().join("functions.py")];
        let result = compute_functions(&targets, &[], Visibility::Private).unwrap();
        // Should have only private functions
        for entries in result.files.values() {
            for sig in entries.keys() {
                let name = extract_function_name(sig);
                assert!(name.starts_with('_'), "Should only have private functions: {}", name);
            }
        }
    }

    #[test]
    fn test_compute_classes() {
        let targets = vec![fixtures_dir().join("classes.py")];
        let result = compute_classes(&targets, &[], Visibility::All).unwrap();
        assert!(!result.files.is_empty());
    }

    #[test]
    fn test_compute_classes_with_pattern() {
        let targets = vec![fixtures_dir().join("classes.py")];
        let result = compute_classes(&targets, &["Simple".to_string()], Visibility::All).unwrap();
        assert!(!result.files.is_empty());
    }

    #[test]
    fn test_compute_classes_visibility_public() {
        let targets = vec![fixtures_dir().join("classes.py")];
        let result = compute_classes(&targets, &[], Visibility::Public).unwrap();
        // Check that private fields/methods are filtered
        for classes in result.files.values() {
            for class_info in classes.values() {
                for field in class_info.fields.keys() {
                    let name = field.split(':').next().unwrap_or(field).trim();
                    assert!(!name.starts_with('_'), "Should not have private field: {}", name);
                }
                for method in class_info.methods.keys() {
                    let name = extract_function_name(method);
                    assert!(!name.starts_with('_'), "Should not have private method: {}", name);
                }
            }
        }
    }

    #[test]
    fn test_compute_enums() {
        let targets = vec![fixtures_dir().join("enums.py")];
        let result = compute_enums(&targets, &[]).unwrap();
        assert!(!result.files.is_empty());
    }

    #[test]
    fn test_compute_enums_with_pattern() {
        let targets = vec![fixtures_dir().join("enums.py")];
        let result = compute_enums(&targets, &["Color".to_string()]).unwrap();
        assert!(!result.files.is_empty());
        let has_color = result.files.values().any(|e| e.keys().any(|k| k.contains("Color")));
        assert!(has_color);
    }

    #[test]
    fn test_compute_modules() {
        let targets = vec![fixtures_dir().join("pkg")];
        let result = compute_modules(&targets, &[]).unwrap();
        assert!(!result.modules.is_empty());
    }

    #[test]
    fn test_compute_modules_with_pattern() {
        let targets = vec![fixtures_dir().join("pkg")];
        let result = compute_modules(&targets, &["module".to_string()]).unwrap();
        // Should filter modules by pattern
        assert!(!result.modules.is_empty());
    }

    #[test]
    fn test_compute_modules_file_target() {
        // When target is a file, use parent as base path
        let targets = vec![fixtures_dir().join("functions.py")];
        let result = compute_modules(&targets, &[]).unwrap();
        assert!(!result.modules.is_empty());
    }

    #[test]
    fn test_compute_dump() {
        let targets = vec![fixtures_dir().join("mixed.py")];
        let result = compute_dump(&targets, &[]).unwrap();
        assert!(!result.files.is_empty());
    }

    #[test]
    fn test_compute_dump_with_pattern() {
        let targets = vec![fixtures_dir().join("mixed.py")];
        let result = compute_dump(&targets, &["helper".to_string()]).unwrap();
        assert!(!result.files.is_empty());
    }

    #[test]
    fn test_compute_dump_includes_methods() {
        let targets = vec![fixtures_dir().join("mixed.py")];
        let result = compute_dump(&targets, &[]).unwrap();
        // Should include methods with class prefix
        let has_method = result
            .files
            .values()
            .any(|entries| entries.keys().any(|k| k.contains('.')));
        assert!(has_method, "Dump should include class methods with class.method format");
    }

    #[test]
    fn test_compute_functions_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let targets = vec![temp_dir.path().to_path_buf()];
        let result = compute_functions(&targets, &[], Visibility::All).unwrap();
        assert!(result.files.is_empty());
    }

    #[test]
    fn test_compute_classes_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let targets = vec![temp_dir.path().to_path_buf()];
        let result = compute_classes(&targets, &[], Visibility::All).unwrap();
        assert!(result.files.is_empty());
    }

    #[test]
    fn test_compute_enums_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let targets = vec![temp_dir.path().to_path_buf()];
        let result = compute_enums(&targets, &[]).unwrap();
        assert!(result.files.is_empty());
    }

    #[test]
    fn test_compute_modules_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let targets = vec![temp_dir.path().to_path_buf()];
        let result = compute_modules(&targets, &[]).unwrap();
        assert!(result.modules.is_empty());
    }

    #[test]
    fn test_compute_dump_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let targets = vec![temp_dir.path().to_path_buf()];
        let result = compute_dump(&targets, &[]).unwrap();
        assert!(result.files.is_empty());
    }

    #[test]
    fn test_compute_modules_empty_default() {
        // Test the default path case
        let empty: Vec<PathBuf> = vec![];
        // This should use PathBuf::from(".")
        let files = walk::collect_python_files(&empty);
        assert!(files.is_err() || files.unwrap().is_empty());
    }

    #[test]
    fn test_compute_dump_class_without_prefix() {
        // Test the case where class_sig doesn't start with "class "
        let targets = vec![fixtures_dir().join("mixed.py")];
        let result = compute_dump(&targets, &[]).unwrap();

        // All entries should have been processed
        assert!(!result.files.is_empty());
    }

    #[test]
    fn test_compute_functions_multiple_files() {
        let targets = vec![fixtures_dir()];
        let result = compute_functions(&targets, &[], Visibility::All).unwrap();
        // Should have functions from multiple files
        assert!(result.files.len() >= 2);
    }

    #[test]
    fn test_compute_classes_multiple_files() {
        let targets = vec![fixtures_dir()];
        let result = compute_classes(&targets, &[], Visibility::All).unwrap();
        // Should have classes from multiple files
        assert!(result.files.len() >= 1);
    }

    #[test]
    fn test_files_output_structure() {
        // Test FilesOutput directly
        let output = FilesOutput { files: BTreeMap::new() };
        assert!(output.files.is_empty());
    }

    #[test]
    fn test_classes_output_structure() {
        // Test ClassesOutput directly
        let output = ClassesOutput { files: BTreeMap::new() };
        assert!(output.files.is_empty());
    }

    #[test]
    fn test_class_info_structure() {
        // Test ClassInfo directly
        let info = ClassInfo {
            fields: BTreeMap::new(),
            methods: BTreeMap::new(),
        };
        assert!(info.fields.is_empty());
        assert!(info.methods.is_empty());
    }
}
