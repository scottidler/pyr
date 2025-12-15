use serde::Serialize;
use std::collections::BTreeMap;

/// Top-level output for functions/enums commands
/// Format:
///   files:
///     <filepath>:
///       <signature>: lineno
#[derive(Debug, Serialize, Default)]
pub struct FilesOutput {
    pub files: BTreeMap<String, BTreeMap<String, usize>>,
}

/// Information about a single class
#[derive(Debug, Serialize, Default, Clone)]
pub struct ClassInfo {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, usize>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub methods: BTreeMap<String, usize>,
}

/// Type alias for class map: class_signature -> ClassInfo
pub type ClassMap = BTreeMap<String, ClassInfo>;

/// Top-level output for classes command
/// Format:
///   files:
///     <filepath>:
///       <class_signature>:
///         fields:
///           <field_name>: lineno
///         methods:
///           <method_signature>: lineno
#[derive(Debug, Serialize, Default)]
pub struct ClassesOutput {
    pub files: BTreeMap<String, ClassMap>,
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

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModuleType {
    Package,
    Module,
}
