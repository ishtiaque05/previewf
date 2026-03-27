use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "previewf",
    version,
    about = "Preview and annotate markdown files"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Serve files on localhost for browser preview
    Serve {
        /// File or directory to serve
        path: PathBuf,

        /// Port to listen on
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
    },

    /// View a markdown file in the terminal
    View {
        /// Markdown file to view
        path: PathBuf,
    },

    /// Extract flags from a markdown file
    Flags {
        /// Markdown file to extract flags from
        path: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { path, port } => {
            println!("Serving {} on port {}", path.display(), port);
            Ok(())
        }
        Commands::View { path } => {
            println!("Viewing {}", path.display());
            Ok(())
        }
        Commands::Flags { path, json } => {
            println!("Extracting flags from {} (json: {})", path.display(), json);
            Ok(())
        }
    }
}
