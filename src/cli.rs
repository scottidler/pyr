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
    Functions,

    /// List all classes with methods and inheritance
    Classes,

    /// List all enum definitions
    Enums,

    /// Show module/package structure
    Modules,

    /// Comprehensive output (functions, classes, enums)
    Dump,
}
