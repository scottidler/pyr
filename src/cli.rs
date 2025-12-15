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
    #[arg(short, long, default_value = ".", global = true)]
    pub paths: Vec<PathBuf>,

    /// Force JSON output (default: YAML, or JSON when not a TTY)
    #[arg(short, long, global = true)]
    pub json: bool,

    /// Sort symbols alphabetically (default: file order by line)
    #[arg(short, long, global = true)]
    pub alphabetical: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// List all functions with signatures and locations
    Function {
        /// Patterns to filter by name (prefix match, then contains)
        #[arg(value_name = "PATTERN")]
        patterns: Vec<String>,
    },

    /// List all classes with methods and inheritance
    Class {
        /// Patterns to filter by name (prefix match, then contains)
        #[arg(value_name = "PATTERN")]
        patterns: Vec<String>,
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
