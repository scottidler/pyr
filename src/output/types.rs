use serde::Serialize;
use std::collections::BTreeMap;

/// Type alias for nested class structure: class_signature -> (method_signature -> line_number)
pub type ClassMethodMap = BTreeMap<String, BTreeMap<String, usize>>;

/// Top-level output for functions/enums commands
/// Format:
///   files:
///     <filepath>:
///       <signature>: lineno
#[derive(Debug, Serialize, Default)]
pub struct FilesOutput {
    pub files: BTreeMap<String, BTreeMap<String, usize>>,
}

/// Top-level output for classes command
/// Format:
///   files:
///     <filepath>:
///       <class_name>:
///         <method_signature>: lineno
#[derive(Debug, Serialize, Default)]
pub struct ClassesOutput {
    pub files: BTreeMap<String, BTreeMap<String, BTreeMap<String, usize>>>,
}

/// Top-level output for modules command
#[derive(Debug, Serialize, Default)]
pub struct ModulesOutput {
    pub modules: BTreeMap<String, ModuleNode>,
}

/// A node in the module tree
#[derive(Debug, Serialize)]
pub struct ModuleNode {
    #[serde(rename = "type")]
    pub node_type: ModuleType,

    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub children: BTreeMap<String, ModuleNode>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleType {
    Package,
    Module,
}
