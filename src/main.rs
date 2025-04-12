use anyhow::Result;
use clap::Parser;
use deloxide::showcase;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Deloxide - Cross-Language Deadlock Detector With Visualization Support"
)]
struct Cli {
    /// Path to the log file
    log_file: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    showcase::showcase(cli.log_file)?;
    Ok(())
}
