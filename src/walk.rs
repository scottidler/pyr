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
