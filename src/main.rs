use clap::{Parser, Subcommand};
use deloxide:: showcase;
use std::path::PathBuf;

#[derive(Parser)]
#[clap(author, version, about = "Deloxide - Deadlock Detection Tool")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Upload log file to showcase visualization
    Showcase {
        /// Path to the log file
        #[clap(required = true)]
        log_file: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Showcase { log_file } => {
            showcase::cli_showcase(log_file)?;
        }
    }

    Ok(())
}