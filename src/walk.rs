use eyre::{Result, WrapErr};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Directories to skip during traversal
const IGNORE_DIRS: &[&str] = &[
    "__pycache__",
    ".git",
    "venv",
    ".venv",
    "node_modules",
    ".tox",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "dist",
    "build",
    "*.egg-info",
];

/// Collect all Python files from the given targets
pub fn collect_python_files(targets: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for target in targets {
        if !target.exists() {
            return Err(eyre::eyre!("Path does not exist: {}", target.display()));
        }

        if target.is_file() {
            if is_python_file(target) {
                files.push(target.clone());
            }
        } else if target.is_dir() {
            collect_from_directory(target, &mut files)
                .wrap_err_with(|| format!("Failed to walk directory: {}", target.display()))?;
        }
    }

    // Sort files alphabetically for deterministic output
    files.sort();
    Ok(files)
}

fn collect_from_directory(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !should_ignore(e.file_name().to_string_lossy().as_ref()))
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && is_python_file(path) {
            files.push(path.to_path_buf());
        }
    }

    Ok(())
}

fn is_python_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "py")
}

fn should_ignore(name: &str) -> bool {
    IGNORE_DIRS.iter().any(|pattern| {
        if let Some(suffix) = pattern.strip_prefix('*') {
            // Simple glob: *.egg-info
            name.ends_with(suffix)
        } else {
            name == *pattern
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn test_is_python_file_true() {
        assert!(is_python_file(Path::new("test.py")));
        assert!(is_python_file(Path::new("/path/to/module.py")));
        assert!(is_python_file(Path::new("__init__.py")));
    }

    #[test]
    fn test_is_python_file_false() {
        assert!(!is_python_file(Path::new("test.txt")));
        assert!(!is_python_file(Path::new("test.pyc")));
        assert!(!is_python_file(Path::new("test.pyi")));
        assert!(!is_python_file(Path::new("test")));
        assert!(!is_python_file(Path::new("")));
    }

    #[test]
    fn test_should_ignore_pycache() {
        assert!(should_ignore("__pycache__"));
    }

    #[test]
    fn test_should_ignore_git() {
        assert!(should_ignore(".git"));
    }

    #[test]
    fn test_should_ignore_venv() {
        assert!(should_ignore("venv"));
        assert!(should_ignore(".venv"));
    }

    #[test]
    fn test_should_ignore_egg_info() {
        assert!(should_ignore("mypackage.egg-info"));
        assert!(should_ignore("test.egg-info"));
    }

    #[test]
    fn test_should_not_ignore_regular_dirs() {
        assert!(!should_ignore("src"));
        assert!(!should_ignore("tests"));
        assert!(!should_ignore("app"));
        assert!(!should_ignore("lib"));
    }

    #[test]
    fn test_collect_python_files_single_file() {
        let path = fixtures_dir().join("functions.py");
        let result = collect_python_files(&[path.clone()]);
        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], path);
    }

    #[test]
    fn test_collect_python_files_directory() {
        let dir = fixtures_dir();
        let result = collect_python_files(&[dir]);
        assert!(result.is_ok());
        let files = result.unwrap();
        // Should find all .py files in fixtures directory
        assert!(files.len() >= 5); // functions.py, classes.py, enums.py, mixed.py, empty.py, etc.
        for file in &files {
            assert!(file.extension().is_some_and(|ext| ext == "py"));
        }
    }

    #[test]
    fn test_collect_python_files_nonexistent() {
        let path = fixtures_dir().join("nonexistent");
        let result = collect_python_files(&[path]);
        assert!(result.is_err());
    }

    #[test]
    fn test_collect_python_files_non_python_file() {
        // Create a temp non-python file
        let temp_dir = tempfile::tempdir().unwrap();
        let txt_file = temp_dir.path().join("test.txt");
        fs::write(&txt_file, "not python").unwrap();

        let result = collect_python_files(&[txt_file]);
        assert!(result.is_ok());
        let files = result.unwrap();
        assert!(files.is_empty()); // Should not include .txt files
    }

    #[test]
    fn test_collect_python_files_sorted() {
        let dir = fixtures_dir();
        let result = collect_python_files(&[dir]);
        assert!(result.is_ok());
        let files = result.unwrap();

        // Check files are sorted
        for i in 1..files.len() {
            assert!(files[i - 1] <= files[i]);
        }
    }

    #[test]
    fn test_collect_python_files_nested() {
        let dir = fixtures_dir().join("pkg");
        let result = collect_python_files(&[dir]);
        assert!(result.is_ok());
        let files = result.unwrap();

        // Should find __init__.py, module.py, subpkg/__init__.py, subpkg/nested.py
        assert!(files.len() >= 4);

        let file_names: Vec<_> = files.iter().map(|f| f.file_name().unwrap().to_str().unwrap()).collect();
        assert!(file_names.contains(&"__init__.py"));
        assert!(file_names.contains(&"module.py"));
        assert!(file_names.contains(&"nested.py"));
    }

    #[test]
    fn test_collect_python_files_ignores_pycache() {
        let temp_dir = tempfile::tempdir().unwrap();
        let pycache = temp_dir.path().join("__pycache__");
        fs::create_dir(&pycache).unwrap();
        fs::write(pycache.join("cached.py"), "# cached").unwrap();
        fs::write(temp_dir.path().join("main.py"), "# main").unwrap();

        let result = collect_python_files(&[temp_dir.path().to_path_buf()]);
        assert!(result.is_ok());
        let files = result.unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap() == "main.py");
    }

    #[test]
    fn test_collect_python_files_multiple_targets() {
        let functions = fixtures_dir().join("functions.py");
        let classes = fixtures_dir().join("classes.py");

        let result = collect_python_files(&[functions.clone(), classes.clone()]);
        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&functions));
        assert!(files.contains(&classes));
    }
}
