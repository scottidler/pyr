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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn test_build_module_tree_empty() {
        let files: Vec<PathBuf> = vec![];
        let base = PathBuf::from("/base");
        let result = build_module_tree(&files, &base);
        assert!(result.modules.is_empty());
    }

    #[test]
    fn test_build_module_tree_single_file() {
        let base = PathBuf::from("/base");
        let files = vec![PathBuf::from("/base/test.py")];
        let result = build_module_tree(&files, &base);

        assert_eq!(result.modules.len(), 1);
        assert!(result.modules.contains_key("test.py"));

        let node = result.modules.get("test.py").unwrap();
        assert_eq!(node.node_type, ModuleType::Module);
        assert!(node.children.is_empty());
    }

    #[test]
    fn test_build_module_tree_nested() {
        let base = PathBuf::from("/base");
        let files = vec![
            PathBuf::from("/base/pkg/__init__.py"),
            PathBuf::from("/base/pkg/module.py"),
        ];
        let result = build_module_tree(&files, &base);

        // Should have "pkg" as a package
        assert!(result.modules.contains_key("pkg"));
        let pkg = result.modules.get("pkg").unwrap();
        assert_eq!(pkg.node_type, ModuleType::Package);

        // pkg should have children
        assert!(pkg.children.contains_key("pkg/__init__.py"));
        assert!(pkg.children.contains_key("pkg/module.py"));
    }

    #[test]
    fn test_build_module_tree_deeply_nested() {
        let base = PathBuf::from("/base");
        let files = vec![PathBuf::from("/base/a/b/c/module.py")];
        let result = build_module_tree(&files, &base);

        // Should have "a" as top-level
        assert!(result.modules.contains_key("a"));
        let a = result.modules.get("a").unwrap();
        assert_eq!(a.node_type, ModuleType::Package);

        // a should have b
        assert!(a.children.contains_key("a/b"));
        let b = a.children.get("a/b").unwrap();
        assert_eq!(b.node_type, ModuleType::Package);

        // b should have c
        assert!(b.children.contains_key("a/b/c"));
        let c = b.children.get("a/b/c").unwrap();
        assert_eq!(c.node_type, ModuleType::Package);

        // c should have module.py
        assert!(c.children.contains_key("a/b/c/module.py"));
        let module = c.children.get("a/b/c/module.py").unwrap();
        assert_eq!(module.node_type, ModuleType::Module);
    }

    #[test]
    fn test_build_module_tree_fixtures() {
        let base = fixtures_dir();
        let files = vec![
            base.join("functions.py"),
            base.join("classes.py"),
            base.join("pkg/__init__.py"),
            base.join("pkg/module.py"),
        ];
        let result = build_module_tree(&files, &base);

        // Should have top-level modules
        assert!(result.modules.contains_key("functions.py"));
        assert!(result.modules.contains_key("classes.py"));

        // And the pkg package
        assert!(result.modules.contains_key("pkg"));
    }

    #[test]
    fn test_insert_path_empty() {
        let mut tree = BTreeMap::new();
        insert_path(&mut tree, "");
        // Empty string splits into [""], which creates a single entry
        // This is acceptable behavior - empty string becomes a single-segment path
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn test_insert_path_single_file() {
        let mut tree = BTreeMap::new();
        insert_path(&mut tree, "module.py");

        assert_eq!(tree.len(), 1);
        let node = tree.get("module.py").unwrap();
        assert_eq!(node.node_type, ModuleType::Module);
    }

    #[test]
    fn test_insert_path_nested() {
        let mut tree = BTreeMap::new();
        insert_path(&mut tree, "pkg/subpkg/module.py");

        // Should create pkg -> subpkg -> module.py
        assert!(tree.contains_key("pkg"));
        let pkg = tree.get("pkg").unwrap();
        assert_eq!(pkg.node_type, ModuleType::Package);

        assert!(pkg.children.contains_key("pkg/subpkg"));
        let subpkg = pkg.children.get("pkg/subpkg").unwrap();
        assert_eq!(subpkg.node_type, ModuleType::Package);

        assert!(subpkg.children.contains_key("pkg/subpkg/module.py"));
        let module = subpkg.children.get("pkg/subpkg/module.py").unwrap();
        assert_eq!(module.node_type, ModuleType::Module);
    }

    #[test]
    fn test_module_type_eq() {
        assert_eq!(ModuleType::Module, ModuleType::Module);
        assert_eq!(ModuleType::Package, ModuleType::Package);
        assert_ne!(ModuleType::Module, ModuleType::Package);
    }
}
