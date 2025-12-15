use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "pyr",
    about = "Fast Python codebase analysis for agentic LLMs",
    version = env!("GIT_DESCRIBE"),
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Files or directories to analyze (default: current directory)
    #[arg(short = 't', long = "target", default_value = ".", global = true)]
    pub targets: Vec<PathBuf>,

    /// Force JSON output (default: YAML, or JSON when not a TTY)
    #[arg(short, long, global = true)]
    pub json: bool,

    /// Sort symbols alphabetically (default: file order by line)
    #[arg(short, long, global = true)]
    pub alphabetical: bool,
}

/// Visibility filter for functions/methods/fields
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Visibility {
    #[default]
    All,
    Public,
    Private,
}

#[derive(Subcommand)]
pub enum Command {
    /// List all functions with signatures and locations
    Function {
        /// Patterns to filter by name (prefix match, then contains)
        #[arg(value_name = "PATTERN")]
        patterns: Vec<String>,

        /// Show only public functions (not starting with _)
        #[arg(long, conflicts_with = "private")]
        public: bool,

        /// Show only private functions (starting with _)
        #[arg(long, conflicts_with = "public")]
        private: bool,
    },

    /// List all classes with methods and inheritance
    Class {
        /// Patterns to filter by name (prefix match, then contains)
        #[arg(value_name = "PATTERN")]
        patterns: Vec<String>,

        /// Show only public fields/methods (not starting with _)
        #[arg(long, conflicts_with = "private")]
        public: bool,

        /// Show only private fields/methods (starting with _)
        #[arg(long, conflicts_with = "public")]
        private: bool,
    },

    /// List all enum definitions
    Enum {
        /// Patterns to filter by name (prefix match, then contains)
        #[arg(value_name = "PATTERN")]
        patterns: Vec<String>,
    },

    /// Show module/package structure
    Module {
        /// Patterns to filter by name (prefix match, then contains)
        #[arg(value_name = "PATTERN")]
        patterns: Vec<String>,
    },

    /// Comprehensive output (functions, classes, enums)
    Dump {
        /// Patterns to filter by name (prefix match, then contains)
        #[arg(value_name = "PATTERN")]
        patterns: Vec<String>,
    },
}

impl Visibility {
    pub fn from_flags(public: bool, private: bool) -> Self {
        match (public, private) {
            (true, false) => Visibility::Public,
            (false, true) => Visibility::Private,
            _ => Visibility::All,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visibility_default() {
        let vis = Visibility::default();
        assert_eq!(vis, Visibility::All);
    }

    #[test]
    fn test_visibility_from_flags_all() {
        assert_eq!(Visibility::from_flags(false, false), Visibility::All);
    }

    #[test]
    fn test_visibility_from_flags_public() {
        assert_eq!(Visibility::from_flags(true, false), Visibility::Public);
    }

    #[test]
    fn test_visibility_from_flags_private() {
        assert_eq!(Visibility::from_flags(false, true), Visibility::Private);
    }

    #[test]
    fn test_visibility_from_flags_both_defaults_to_all() {
        // Both true is invalid but should default to All
        assert_eq!(Visibility::from_flags(true, true), Visibility::All);
    }

    #[test]
    fn test_visibility_eq() {
        assert_eq!(Visibility::All, Visibility::All);
        assert_eq!(Visibility::Public, Visibility::Public);
        assert_eq!(Visibility::Private, Visibility::Private);
        assert_ne!(Visibility::All, Visibility::Public);
        assert_ne!(Visibility::Public, Visibility::Private);
    }

    #[test]
    fn test_visibility_clone() {
        let vis = Visibility::Public;
        let cloned = vis;
        assert_eq!(vis, cloned);
    }

    #[test]
    fn test_visibility_debug() {
        assert_eq!(format!("{:?}", Visibility::All), "All");
        assert_eq!(format!("{:?}", Visibility::Public), "Public");
        assert_eq!(format!("{:?}", Visibility::Private), "Private");
    }
}
