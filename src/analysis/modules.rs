use crate::output::{ModuleNode, ModuleType, ModulesOutput};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Build a module tree from collected Python files
pub fn build_module_tree(files: &[PathBuf], base_path: &Path) -> ModulesOutput {
    let mut output = ModulesOutput::default();

    for file in files {
        // Get path relative to base
        let rel_path = file
            .strip_prefix(base_path)
            .unwrap_or(file)
            .to_string_lossy()
            .to_string();

        insert_path(&mut output.modules, &rel_path);
    }

    output
}

/// Insert a file path into the module tree
fn insert_path(tree: &mut BTreeMap<String, ModuleNode>, path: &str) {
    let parts: Vec<&str> = path.split('/').collect();

    if parts.is_empty() {
        return;
    }

    let mut current = tree;

    for (i, _part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        let path_so_far = parts[..=i].join("/");

        if is_last {
            // This is a file (module)
            current.insert(
                path_so_far,
                ModuleNode {
                    node_type: ModuleType::Module,
                    children: BTreeMap::new(),
                },
            );
        } else {
            // This is a directory (package)
            let entry = current.entry(path_so_far.clone()).or_insert_with(|| ModuleNode {
                node_type: ModuleType::Package,
                children: BTreeMap::new(),
            });
            current = &mut entry.children;
        }
    }
}
