use eyre::Result;
use serde::Serialize;
use std::io::{self, IsTerminal, Write};

/// Determines output format based on flags and TTY detection
pub fn should_use_json(json_flag: bool) -> bool {
    json_flag || !io::stdout().is_terminal()
}

/// Outputs serializable data as YAML or JSON
pub fn output<T: Serialize>(data: &T, use_json: bool) -> Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    if use_json {
        serde_json::to_writer_pretty(&mut handle, data)?;
        writeln!(handle)?;
    } else {
        serde_yaml::to_writer(&mut handle, data)?;
    }

    Ok(())
}
