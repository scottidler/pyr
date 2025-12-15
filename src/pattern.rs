use crate::output::{ModuleNode, ModulesOutput};
use std::collections::BTreeMap;

/// Extract the subject name from a function signature
/// "def compute_total(x: int) -> int" -> "compute_total"
/// "async def fetch_data() -> None" -> "fetch_data"
pub fn extract_function_name(signature: &str) -> &str {
    // Skip "async " if present, then skip "def "
    let s = signature.strip_prefix("async ").unwrap_or(signature);
    let s = s.strip_prefix("def ").unwrap_or(s);

    // Take everything up to the first '('
    s.split('(').next().unwrap_or(s).trim()
}

/// Extract the subject name from a class/enum signature
/// "class UserService" -> "UserService"
/// "class UserService(BaseService)" -> "UserService"
pub fn extract_class_name(signature: &str) -> &str {
    let s = signature.strip_prefix("class ").unwrap_or(signature);

    // Take everything up to the first '(' or end of string
    s.split('(').next().unwrap_or(s).trim()
}

/// Extract the subject name from a dump signature (handles functions, class.method, and enums)
/// "def compute_total(x: int) -> int" -> "compute_total"
/// "UserService.def create_user(self) -> User" -> "create_user"
/// "class OrderStatus(Enum)" -> "OrderStatus"
pub fn extract_dump_name(signature: &str) -> &str {
    // Check if it's a class method (contains ".")
    if let Some(dot_pos) = signature.find('.') {
        // It's "ClassName.def method_name(...)" or "ClassName.async def method_name(...)"
        let method_part = &signature[dot_pos + 1..];
        return extract_function_name(method_part);
    }

    // Check if it's a class/enum
    if signature.starts_with("class ") {
        return extract_class_name(signature);
    }

    // It's a function
    extract_function_name(signature)
}

/// Extract the module name from a path
/// "src/utils/helpers.py" -> "helpers.py"
/// "src/models" -> "models"
pub fn extract_module_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Match priority levels (from highest to lowest priority)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MatchLevel {
    StartsWithCaseSensitive,
    StartsWithCaseInsensitive,
    ContainsCaseSensitive,
    ContainsCaseInsensitive,
    NoMatch,
}

/// Check how well a pattern matches a subject
fn match_level(subject: &str, pattern: &str) -> MatchLevel {
    // 1. startswith case sensitive
    if subject.starts_with(pattern) {
        return MatchLevel::StartsWithCaseSensitive;
    }

    let subject_lower = subject.to_lowercase();
    let pattern_lower = pattern.to_lowercase();

    // 2. startswith case insensitive
    if subject_lower.starts_with(&pattern_lower) {
        return MatchLevel::StartsWithCaseInsensitive;
    }

    // 3. contains case sensitive
    if subject.contains(pattern) {
        return MatchLevel::ContainsCaseSensitive;
    }

    // 4. contains case insensitive
    if subject_lower.contains(&pattern_lower) {
        return MatchLevel::ContainsCaseInsensitive;
    }

    MatchLevel::NoMatch
}

/// For a single pattern, find the best (highest priority) match level that has any matches
/// among the given subjects. Computes all match levels in parallel, then applies in order.
fn find_best_match_level_for_pattern<'a>(
    subjects: impl Iterator<Item = &'a str>,
    pattern: &str,
) -> (MatchLevel, Vec<&'a str>) {
    // Compute match levels for all subjects in one pass
    let mut by_level: [Vec<&'a str>; 4] = Default::default();

    for subject in subjects {
        match match_level(subject, pattern) {
            MatchLevel::StartsWithCaseSensitive => by_level[0].push(subject),
            MatchLevel::StartsWithCaseInsensitive => by_level[1].push(subject),
            MatchLevel::ContainsCaseSensitive => by_level[2].push(subject),
            MatchLevel::ContainsCaseInsensitive => by_level[3].push(subject),
            MatchLevel::NoMatch => {}
        }
    }

    // Apply in priority order, return first non-empty
    let levels = [
        MatchLevel::StartsWithCaseSensitive,
        MatchLevel::StartsWithCaseInsensitive,
        MatchLevel::ContainsCaseSensitive,
        MatchLevel::ContainsCaseInsensitive,
    ];

    for (i, level) in levels.into_iter().enumerate() {
        if !by_level[i].is_empty() {
            return (level, std::mem::take(&mut by_level[i]));
        }
    }

    (MatchLevel::NoMatch, vec![])
}

/// Filter files output (file -> (signature -> line)) by patterns.
/// Applies cascading match logic GLOBALLY across all files, not per-file.
pub fn filter_files_output<F>(
    files: BTreeMap<String, BTreeMap<String, usize>>,
    patterns: &[String],
    name_extractor: F,
) -> BTreeMap<String, BTreeMap<String, usize>>
where
    F: Fn(&str) -> &str + Copy,
{
    if patterns.is_empty() {
        return files;
    }

    // Flatten all entries: (file_path, signature, line, extracted_name)
    let all_entries: Vec<(String, String, usize, String)> = files
        .into_iter()
        .flat_map(|(file_path, entries)| {
            entries.into_iter().map(move |(sig, line)| {
                let name = name_extractor(&sig).to_string();
                (file_path.clone(), sig, line, name)
            })
        })
        .collect();

    // For each pattern, find globally which names match at the best level
    let mut matching_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for pattern in patterns {
        let subjects = all_entries.iter().map(|(_, _, _, name)| name.as_str());
        let (_, matched_names) = find_best_match_level_for_pattern(subjects, pattern);

        for matched in matched_names {
            matching_names.insert(matched.to_string());
        }
    }

    // Re-group by file, filtering to only matching names
    let mut result: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();

    for (file_path, sig, line, name) in all_entries {
        if matching_names.contains(&name) {
            result.entry(file_path).or_default().insert(sig, line);
        }
    }

    result
}

/// Filter classes output (file -> (class_sig -> (method_sig -> line))) by patterns.
/// Applies cascading match logic GLOBALLY across all files, not per-file.
pub fn filter_classes_output(
    files: BTreeMap<String, BTreeMap<String, BTreeMap<String, usize>>>,
    patterns: &[String],
) -> BTreeMap<String, BTreeMap<String, BTreeMap<String, usize>>> {
    if patterns.is_empty() {
        return files;
    }

    // Flatten all class entries: (file_path, class_sig, methods, extracted_name)
    let all_entries: Vec<(String, String, BTreeMap<String, usize>, String)> = files
        .into_iter()
        .flat_map(|(file_path, classes)| {
            classes.into_iter().map(move |(class_sig, methods)| {
                let name = extract_class_name(&class_sig).to_string();
                (file_path.clone(), class_sig, methods, name)
            })
        })
        .collect();

    // For each pattern, find globally which names match at the best level
    let mut matching_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for pattern in patterns {
        let subjects = all_entries.iter().map(|(_, _, _, name)| name.as_str());
        let (_, matched_names) = find_best_match_level_for_pattern(subjects, pattern);

        for matched in matched_names {
            matching_names.insert(matched.to_string());
        }
    }

    // Re-group by file, filtering to only matching names
    let mut result: BTreeMap<String, BTreeMap<String, BTreeMap<String, usize>>> = BTreeMap::new();

    for (file_path, class_sig, methods, name) in all_entries {
        if matching_names.contains(&name) {
            result.entry(file_path).or_default().insert(class_sig, methods);
        }
    }

    result
}

/// Filter modules output by patterns (matches against module/package names)
pub fn filter_modules_output(output: ModulesOutput, patterns: &[String]) -> ModulesOutput {
    if patterns.is_empty() {
        return output;
    }

    ModulesOutput {
        modules: filter_module_tree(output.modules, patterns),
    }
}

/// Recursively filter a module tree by patterns using cascading match logic
fn filter_module_tree(tree: BTreeMap<String, ModuleNode>, patterns: &[String]) -> BTreeMap<String, ModuleNode> {
    // Collect all module names at this level for cascading match
    let entries: Vec<(String, ModuleNode, String)> = tree
        .into_iter()
        .map(|(path, node)| {
            let name = extract_module_name(&path);
            let name = name.strip_suffix(".py").unwrap_or(name).to_string();
            (path, node, name)
        })
        .collect();

    // For each pattern, find which modules match at the best level
    let mut matching_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    for pattern in patterns {
        let subjects = entries.iter().map(|(_, _, name)| name.as_str());
        let (_, matched_names) = find_best_match_level_for_pattern(subjects, pattern);

        for (path, _, name) in &entries {
            if matched_names.contains(&name.as_str()) {
                matching_paths.insert(path.clone());
            }
        }
    }

    entries
        .into_iter()
        .filter_map(|(path, mut node, _)| {
            // Filter children recursively
            node.children = filter_module_tree(node.children, patterns);

            // Keep this node if:
            // 1. Its name matches any pattern at the best level, OR
            // 2. It has matching children (packages with matching descendants)
            if matching_paths.contains(&path) || !node.children.is_empty() {
                Some((path, node))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::ModuleType;

    // ==================== Name Extraction Tests ====================

    #[test]
    fn test_extract_function_name_sync() {
        assert_eq!(
            extract_function_name("def compute_total(x: int) -> int"),
            "compute_total"
        );
        assert_eq!(extract_function_name("def simple()"), "simple");
        assert_eq!(
            extract_function_name("def with_defaults(a: int = 5) -> None"),
            "with_defaults"
        );
        assert_eq!(extract_function_name("def _private_func()"), "_private_func");
        assert_eq!(extract_function_name("def __dunder__()"), "__dunder__");
    }

    #[test]
    fn test_extract_function_name_async() {
        assert_eq!(extract_function_name("async def fetch_data() -> None"), "fetch_data");
        assert_eq!(
            extract_function_name("async def async_handler(req: Request) -> Response"),
            "async_handler"
        );
    }

    #[test]
    fn test_extract_class_name_simple() {
        assert_eq!(extract_class_name("class UserService"), "UserService");
        assert_eq!(extract_class_name("class _PrivateClass"), "_PrivateClass");
    }

    #[test]
    fn test_extract_class_name_with_bases() {
        assert_eq!(extract_class_name("class UserService(BaseService)"), "UserService");
        assert_eq!(extract_class_name("class OrderStatus(Enum)"), "OrderStatus");
        assert_eq!(extract_class_name("class Multi(Base1, Base2, Base3)"), "Multi");
    }

    #[test]
    fn test_extract_dump_name_functions() {
        // Regular functions
        assert_eq!(extract_dump_name("def compute_total(x: int) -> int"), "compute_total");
        assert_eq!(extract_dump_name("async def fetch_data() -> None"), "fetch_data");
    }

    #[test]
    fn test_extract_dump_name_class_methods() {
        // Class methods (ClassName.def method_name format)
        assert_eq!(
            extract_dump_name("UserService.def create_user(self) -> User"),
            "create_user"
        );
        assert_eq!(
            extract_dump_name("UserService.async def fetch_user(self, id: int) -> User"),
            "fetch_user"
        );
        assert_eq!(extract_dump_name("MyClass.def __init__(self)"), "__init__");
    }

    #[test]
    fn test_extract_dump_name_enums() {
        assert_eq!(extract_dump_name("class OrderStatus(Enum)"), "OrderStatus");
        assert_eq!(extract_dump_name("class Color(IntEnum)"), "Color");
    }

    #[test]
    fn test_extract_module_name() {
        assert_eq!(extract_module_name("src/utils/helpers.py"), "helpers.py");
        assert_eq!(extract_module_name("src/models"), "models");
        assert_eq!(extract_module_name("single_file.py"), "single_file.py");
        assert_eq!(extract_module_name("deeply/nested/path/to/module.py"), "module.py");
    }

    // ==================== Match Level Priority Tests ====================

    #[test]
    fn test_match_level_startswith_case_sensitive() {
        assert_eq!(
            match_level("compute_total", "comp"),
            MatchLevel::StartsWithCaseSensitive
        );
        assert_eq!(
            match_level("compute_total", "compute"),
            MatchLevel::StartsWithCaseSensitive
        );
        assert_eq!(
            match_level("compute_total", "compute_total"),
            MatchLevel::StartsWithCaseSensitive
        );
        // Empty pattern always matches as startswith
        assert_eq!(match_level("compute_total", ""), MatchLevel::StartsWithCaseSensitive);
    }

    #[test]
    fn test_match_level_startswith_case_insensitive() {
        assert_eq!(
            match_level("compute_total", "Comp"),
            MatchLevel::StartsWithCaseInsensitive
        );
        assert_eq!(
            match_level("compute_total", "COMP"),
            MatchLevel::StartsWithCaseInsensitive
        );
        assert_eq!(
            match_level("compute_total", "CoMpUtE"),
            MatchLevel::StartsWithCaseInsensitive
        );
        assert_eq!(
            match_level("UserService", "user"),
            MatchLevel::StartsWithCaseInsensitive
        );
    }

    #[test]
    fn test_match_level_contains_case_sensitive() {
        assert_eq!(match_level("compute_total", "pute"), MatchLevel::ContainsCaseSensitive);
        assert_eq!(
            match_level("compute_total", "_total"),
            MatchLevel::ContainsCaseSensitive
        );
        assert_eq!(match_level("compute_total", "otal"), MatchLevel::ContainsCaseSensitive);
        assert_eq!(match_level("get_user_by_id", "user"), MatchLevel::ContainsCaseSensitive);
    }

    #[test]
    fn test_match_level_contains_case_insensitive() {
        assert_eq!(
            match_level("compute_total", "PUTE"),
            MatchLevel::ContainsCaseInsensitive
        );
        assert_eq!(
            match_level("compute_total", "TOTAL"),
            MatchLevel::ContainsCaseInsensitive
        );
        assert_eq!(match_level("getUserById", "USER"), MatchLevel::ContainsCaseInsensitive);
    }

    #[test]
    fn test_match_level_no_match() {
        assert_eq!(match_level("compute_total", "xyz"), MatchLevel::NoMatch);
        assert_eq!(match_level("compute_total", "totals"), MatchLevel::NoMatch);
        assert_eq!(match_level("short", "longer_pattern"), MatchLevel::NoMatch);
    }

    #[test]
    fn test_match_level_priority_ordering() {
        // Verify the priority order: case-sensitive startswith beats case-insensitive startswith
        assert!(MatchLevel::StartsWithCaseSensitive < MatchLevel::StartsWithCaseInsensitive);
        assert!(MatchLevel::StartsWithCaseInsensitive < MatchLevel::ContainsCaseSensitive);
        assert!(MatchLevel::ContainsCaseSensitive < MatchLevel::ContainsCaseInsensitive);
        assert!(MatchLevel::ContainsCaseInsensitive < MatchLevel::NoMatch);
    }

    // ==================== Cascading Match Logic Tests ====================

    /// Helper to wrap a single file's functions for testing
    fn wrap_in_file(funcs: BTreeMap<String, usize>) -> BTreeMap<String, BTreeMap<String, usize>> {
        let mut files = BTreeMap::new();
        files.insert("test.py".to_string(), funcs);
        files
    }

    /// Helper to get functions from the test file
    fn get_test_file(files: &BTreeMap<String, BTreeMap<String, usize>>) -> &BTreeMap<String, usize> {
        files.get("test.py").unwrap()
    }

    #[test]
    fn test_cascading_match_prefers_startswith() {
        // When pattern "test" is used:
        // - "test_function" matches at StartsWithCaseSensitive
        // - "_test_helper" matches at ContainsCaseSensitive
        // Only "test_function" should be returned because startswith is preferred

        let mut map = BTreeMap::new();
        map.insert("def test_function() -> None".to_string(), 10);
        map.insert("def _test_helper() -> None".to_string(), 20);
        map.insert("def other() -> None".to_string(), 30);

        let patterns = vec!["test".to_string()];
        let filtered = filter_files_output(wrap_in_file(map), &patterns, extract_function_name);
        let funcs = get_test_file(&filtered);

        assert_eq!(funcs.len(), 1);
        assert!(funcs.contains_key("def test_function() -> None"));
        assert!(!funcs.contains_key("def _test_helper() -> None"));
    }

    #[test]
    fn test_cascading_falls_back_to_contains_when_no_startswith() {
        // When pattern "helper" is used and no function starts with "helper",
        // it should fall back to contains matching

        let mut map = BTreeMap::new();
        map.insert("def _test_helper() -> None".to_string(), 10);
        map.insert("def my_helper_func() -> None".to_string(), 20);
        map.insert("def other() -> None".to_string(), 30);

        let patterns = vec!["helper".to_string()];
        let filtered = filter_files_output(wrap_in_file(map), &patterns, extract_function_name);
        let funcs = get_test_file(&filtered);

        // Both should match via contains since neither starts with "helper"
        assert_eq!(funcs.len(), 2);
        assert!(funcs.contains_key("def _test_helper() -> None"));
        assert!(funcs.contains_key("def my_helper_func() -> None"));
    }

    #[test]
    fn test_cascading_case_insensitive_startswith() {
        // Pattern "Test" should match "test_func" via case-insensitive startswith
        // but NOT "_test_helper" (which would only match via contains)

        let mut map = BTreeMap::new();
        map.insert("def test_func() -> None".to_string(), 10);
        map.insert("def _test_helper() -> None".to_string(), 20);

        let patterns = vec!["Test".to_string()];
        let filtered = filter_files_output(wrap_in_file(map), &patterns, extract_function_name);
        let funcs = get_test_file(&filtered);

        assert_eq!(funcs.len(), 1);
        assert!(funcs.contains_key("def test_func() -> None"));
    }

    #[test]
    fn test_cascading_multiple_patterns_independent() {
        // Multiple patterns are evaluated independently
        // Pattern "test" -> startswith matches "test_a"
        // Pattern "comp" -> startswith matches "compute"
        // "_test_b" should NOT be included (only matches via contains)

        let mut map = BTreeMap::new();
        map.insert("def test_a() -> None".to_string(), 10);
        map.insert("def _test_b() -> None".to_string(), 20);
        map.insert("def compute() -> None".to_string(), 30);
        map.insert("def other() -> None".to_string(), 40);

        let patterns = vec!["test".to_string(), "comp".to_string()];
        let filtered = filter_files_output(wrap_in_file(map), &patterns, extract_function_name);
        let funcs = get_test_file(&filtered);

        assert_eq!(funcs.len(), 2);
        assert!(funcs.contains_key("def test_a() -> None"));
        assert!(funcs.contains_key("def compute() -> None"));
        assert!(!funcs.contains_key("def _test_b() -> None"));
    }

    #[test]
    fn test_cascading_only_contains_matches() {
        // When a pattern only has contains matches, those are returned

        let mut map = BTreeMap::new();
        map.insert("def _internal_validator() -> None".to_string(), 10);
        map.insert("def my_validator_func() -> None".to_string(), 20);
        map.insert("def other() -> None".to_string(), 30);

        let patterns = vec!["validator".to_string()];
        let filtered = filter_files_output(wrap_in_file(map), &patterns, extract_function_name);
        let funcs = get_test_file(&filtered);

        // Both have "validator" via contains (neither starts with it)
        assert_eq!(funcs.len(), 2);
        assert!(funcs.contains_key("def _internal_validator() -> None"));
        assert!(funcs.contains_key("def my_validator_func() -> None"));
    }

    #[test]
    fn test_cascading_no_matches() {
        let mut map = BTreeMap::new();
        map.insert("def foo() -> None".to_string(), 10);
        map.insert("def bar() -> None".to_string(), 20);

        let patterns = vec!["xyz".to_string()];
        let filtered = filter_files_output(wrap_in_file(map), &patterns, extract_function_name);

        assert!(filtered.is_empty());
    }

    #[test]
    fn test_cascading_global_across_files() {
        // Key test: cascading should be global across all files
        // File1 has "test_foo" (startswith match)
        // File2 has "_test_bar" (contains match)
        // Since there's a startswith match globally, contains matches should be excluded

        let mut files = BTreeMap::new();

        let mut file1 = BTreeMap::new();
        file1.insert("def test_foo() -> None".to_string(), 10);
        files.insert("file1.py".to_string(), file1);

        let mut file2 = BTreeMap::new();
        file2.insert("def _test_bar() -> None".to_string(), 20);
        files.insert("file2.py".to_string(), file2);

        let patterns = vec!["test".to_string()];
        let filtered = filter_files_output(files, &patterns, extract_function_name);

        // Only file1.py should be present (file2.py filtered out entirely)
        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains_key("file1.py"));
        assert!(!filtered.contains_key("file2.py"));
    }

    // ==================== Files Output Filter Tests ====================

    fn make_files_output() -> BTreeMap<String, BTreeMap<String, usize>> {
        let mut files = BTreeMap::new();

        let mut file1 = BTreeMap::new();
        file1.insert("def compute_total(x: int) -> int".to_string(), 10);
        file1.insert("def print_summary() -> None".to_string(), 20);
        files.insert("src/billing.py".to_string(), file1);

        let mut file2 = BTreeMap::new();
        file2.insert("def validate_email(email: str) -> bool".to_string(), 10);
        file2.insert("def compute_hash(data: str) -> str".to_string(), 20);
        files.insert("src/utils.py".to_string(), file2);

        let mut file3 = BTreeMap::new();
        file3.insert("def hello() -> str".to_string(), 10);
        files.insert("src/greet.py".to_string(), file3);

        files
    }

    #[test]
    fn test_filter_files_output_no_patterns() {
        let files = make_files_output();
        let original_len = files.len();
        let filtered = filter_files_output(files, &[], extract_function_name);
        assert_eq!(filtered.len(), original_len);
    }

    #[test]
    fn test_filter_files_output_removes_empty_files() {
        let files = make_files_output();
        let patterns = vec!["hello".to_string()];
        let filtered = filter_files_output(files, &patterns, extract_function_name);

        // Only greet.py should remain (has hello function)
        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains_key("src/greet.py"));
    }

    #[test]
    fn test_filter_files_output_multiple_files_partial_match() {
        let files = make_files_output();
        let patterns = vec!["compute".to_string()];
        let filtered = filter_files_output(files, &patterns, extract_function_name);

        // billing.py and utils.py both have compute* functions
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains_key("src/billing.py"));
        assert!(filtered.contains_key("src/utils.py"));

        // Each file should only have the matching functions
        assert_eq!(filtered["src/billing.py"].len(), 1);
        assert!(filtered["src/billing.py"].contains_key("def compute_total(x: int) -> int"));

        assert_eq!(filtered["src/utils.py"].len(), 1);
        assert!(filtered["src/utils.py"].contains_key("def compute_hash(data: str) -> str"));
    }

    // ==================== Classes Output Filter Tests ====================

    fn make_classes_output() -> BTreeMap<String, BTreeMap<String, BTreeMap<String, usize>>> {
        let mut files = BTreeMap::new();

        let mut file1_classes = BTreeMap::new();
        let mut user_methods = BTreeMap::new();
        user_methods.insert("def create(self) -> User".to_string(), 10);
        file1_classes.insert("class UserService".to_string(), user_methods);

        let mut admin_methods = BTreeMap::new();
        admin_methods.insert("def delete(self) -> None".to_string(), 20);
        file1_classes.insert("class AdminService".to_string(), admin_methods);

        files.insert("src/services.py".to_string(), file1_classes);

        let mut file2_classes = BTreeMap::new();
        let mut product_methods = BTreeMap::new();
        product_methods.insert("def list(self) -> list".to_string(), 10);
        file2_classes.insert("class ProductManager".to_string(), product_methods);
        files.insert("src/products.py".to_string(), file2_classes);

        files
    }

    #[test]
    fn test_filter_classes_output_no_patterns() {
        let files = make_classes_output();
        let original_len = files.len();
        let filtered = filter_classes_output(files, &[]);
        assert_eq!(filtered.len(), original_len);
    }

    #[test]
    fn test_filter_classes_output_single_pattern() {
        let files = make_classes_output();
        let patterns = vec!["User".to_string()];
        let filtered = filter_classes_output(files, &patterns);

        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains_key("src/services.py"));
        assert_eq!(filtered["src/services.py"].len(), 1);
        assert!(filtered["src/services.py"].contains_key("class UserService"));
    }

    #[test]
    fn test_filter_classes_output_removes_empty_files() {
        let files = make_classes_output();
        let patterns = vec!["Product".to_string()];
        let filtered = filter_classes_output(files, &patterns);

        // Only products.py should remain
        assert_eq!(filtered.len(), 1);
        assert!(filtered.contains_key("src/products.py"));
    }

    // ==================== Module Tree Filter Tests ====================

    fn make_module_tree() -> ModulesOutput {
        let mut modules = BTreeMap::new();

        // src/
        //   utils/
        //     helpers.py
        //     validators.py
        //   models/
        //     user.py
        //     product.py
        //   main.py

        let mut helpers_children = BTreeMap::new();
        helpers_children.insert(
            "src/utils/helpers.py".to_string(),
            ModuleNode {
                node_type: ModuleType::Module,
                children: BTreeMap::new(),
            },
        );
        helpers_children.insert(
            "src/utils/validators.py".to_string(),
            ModuleNode {
                node_type: ModuleType::Module,
                children: BTreeMap::new(),
            },
        );

        let mut models_children = BTreeMap::new();
        models_children.insert(
            "src/models/user.py".to_string(),
            ModuleNode {
                node_type: ModuleType::Module,
                children: BTreeMap::new(),
            },
        );
        models_children.insert(
            "src/models/product.py".to_string(),
            ModuleNode {
                node_type: ModuleType::Module,
                children: BTreeMap::new(),
            },
        );

        let mut src_children = BTreeMap::new();
        src_children.insert(
            "src/utils".to_string(),
            ModuleNode {
                node_type: ModuleType::Package,
                children: helpers_children,
            },
        );
        src_children.insert(
            "src/models".to_string(),
            ModuleNode {
                node_type: ModuleType::Package,
                children: models_children,
            },
        );
        src_children.insert(
            "src/main.py".to_string(),
            ModuleNode {
                node_type: ModuleType::Module,
                children: BTreeMap::new(),
            },
        );

        modules.insert(
            "src".to_string(),
            ModuleNode {
                node_type: ModuleType::Package,
                children: src_children,
            },
        );

        ModulesOutput { modules }
    }

    #[test]
    fn test_filter_modules_output_no_patterns() {
        let output = make_module_tree();
        let filtered = filter_modules_output(output, &[]);

        // All modules should be present
        assert!(filtered.modules.contains_key("src"));
    }

    #[test]
    fn test_filter_modules_output_leaf_match() {
        let output = make_module_tree();
        let patterns = vec!["helpers".to_string()];
        let filtered = filter_modules_output(output, &patterns);

        // Should have src -> utils -> helpers.py
        assert!(filtered.modules.contains_key("src"));
        let src = &filtered.modules["src"];
        assert!(src.children.contains_key("src/utils"));
        let utils = &src.children["src/utils"];
        assert!(utils.children.contains_key("src/utils/helpers.py"));
        // validators.py should be filtered out
        assert!(!utils.children.contains_key("src/utils/validators.py"));
    }

    #[test]
    fn test_filter_modules_output_package_match() {
        let output = make_module_tree();
        let patterns = vec!["models".to_string()];
        let filtered = filter_modules_output(output, &patterns);

        // Should have src -> models (with all children since models matches)
        assert!(filtered.modules.contains_key("src"));
        let src = &filtered.modules["src"];
        assert!(src.children.contains_key("src/models"));
    }

    #[test]
    fn test_filter_modules_output_case_insensitive() {
        let output = make_module_tree();
        let patterns = vec!["USER".to_string()];
        let filtered = filter_modules_output(output, &patterns);

        // Should find user.py
        assert!(filtered.modules.contains_key("src"));
        let src = &filtered.modules["src"];
        assert!(src.children.contains_key("src/models"));
        let models = &src.children["src/models"];
        assert!(models.children.contains_key("src/models/user.py"));
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_empty_subject() {
        assert_eq!(match_level("", "pattern"), MatchLevel::NoMatch);
        assert_eq!(match_level("", ""), MatchLevel::StartsWithCaseSensitive);
    }

    #[test]
    fn test_unicode_in_names() {
        // Python allows unicode identifiers
        assert_eq!(match_level("calculate_π", "π"), MatchLevel::ContainsCaseSensitive);
        assert_eq!(match_level("计算", "计"), MatchLevel::StartsWithCaseSensitive);
    }

    #[test]
    fn test_special_characters_in_names() {
        // Underscores are common in Python
        assert_eq!(match_level("__init__", "__"), MatchLevel::StartsWithCaseSensitive);
        assert_eq!(match_level("_private", "_priv"), MatchLevel::StartsWithCaseSensitive);
    }

    #[test]
    fn test_full_name_match() {
        // Exact match should be StartsWithCaseSensitive (full string starts with full string)
        assert_eq!(match_level("compute", "compute"), MatchLevel::StartsWithCaseSensitive);
    }

    #[test]
    fn test_pattern_longer_than_subject() {
        assert_eq!(match_level("abc", "abcdef"), MatchLevel::NoMatch);
    }
}
