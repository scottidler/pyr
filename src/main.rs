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
mod walk;

use cli::{Cli, Command};
use output::{ClassMethodMap, ClassesOutput, FilesOutput, output, should_use_json};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let use_json = should_use_json(cli.json);
    let paths = &cli.paths;

    match &cli.command {
        Command::Functions => run_functions(paths, cli.alphabetical, use_json),
        Command::Classes => run_classes(paths, cli.alphabetical, use_json),
        Command::Enums => run_enums(paths, cli.alphabetical, use_json),
        Command::Modules => run_modules(paths, use_json),
        Command::Dump => run_dump(paths, cli.alphabetical, use_json),
    }
}

fn run_functions(targets: &[PathBuf], _alphabetical: bool, use_json: bool) -> Result<()> {
    let files = walk::collect_python_files(targets)?;
    let result = FilesOutput {
        files: process_files_parallel(&files, |path| {
            let functions = analysis::extract_functions(path).ok()?;
            if functions.is_empty() { None } else { Some(functions) }
        }),
    };
    output(&result, use_json)
}

fn run_classes(targets: &[PathBuf], _alphabetical: bool, use_json: bool) -> Result<()> {
    let files = walk::collect_python_files(targets)?;
    let result = ClassesOutput {
        files: process_classes_parallel(&files, |path| {
            let classes = analysis::extract_classes(path).ok()?;
            if classes.is_empty() { None } else { Some(classes) }
        }),
    };
    output(&result, use_json)
}

fn run_enums(targets: &[PathBuf], _alphabetical: bool, use_json: bool) -> Result<()> {
    let files = walk::collect_python_files(targets)?;
    let result = FilesOutput {
        files: process_files_parallel(&files, |path| {
            let enums = analysis::extract_enums(path).ok()?;
            if enums.is_empty() { None } else { Some(enums) }
        }),
    };
    output(&result, use_json)
}

fn run_modules(targets: &[PathBuf], use_json: bool) -> Result<()> {
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
    output(&result, use_json)
}

fn run_dump(targets: &[PathBuf], _alphabetical: bool, use_json: bool) -> Result<()> {
    let files = walk::collect_python_files(targets)?;
    let result = FilesOutput {
        files: process_files_parallel(&files, |path| {
            let mut all_entries = BTreeMap::new();

            if let Ok(functions) = analysis::extract_functions(path) {
                all_entries.extend(functions);
            }
            // Flatten classes: prefix method signatures with class name
            if let Ok(classes) = analysis::extract_classes(path) {
                for (class_sig, methods) in classes {
                    // Extract class name from signature (e.g., "class Foo" -> "Foo")
                    let class_name = class_sig
                        .strip_prefix("class ")
                        .and_then(|s| s.split('(').next())
                        .unwrap_or(&class_sig);
                    for (method_sig, line) in methods {
                        let full_sig = format!("{}.{}", class_name, method_sig);
                        all_entries.insert(full_sig, line);
                    }
                }
            }
            if let Ok(enums) = analysis::extract_enums(path) {
                all_entries.extend(enums);
            }

            if all_entries.is_empty() { None } else { Some(all_entries) }
        }),
    };
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
fn process_classes_parallel<F>(files: &[PathBuf], processor: F) -> BTreeMap<String, ClassMethodMap>
where
    F: Fn(&std::path::Path) -> Option<ClassMethodMap> + Sync,
{
    let results: Mutex<BTreeMap<String, ClassMethodMap>> = Mutex::new(BTreeMap::new());

    files.par_iter().for_each(|path| {
        if let Some(content) = processor(path) {
            let key = path.to_string_lossy().to_string();
            results.lock().unwrap().insert(key, content);
        }
    });

    results.into_inner().unwrap()
}
