use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use previewf::flags::{extract_flags, format_flags_text, FlagReport};
use previewf::server::ServerBuilder;
use previewf::terminal::render_terminal;

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
            let config = ServerBuilder::new()
                .path(&path)
                .port(port)
                .live_reload(true)
                .build()
                .context("Failed to configure server")?;

            previewf::server::run(config)
                .await
                .context("Server error")?;
        }
        Commands::View { path } => {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Cannot read file: {}", path.display()))?;

            let rendered = render_terminal(&content);
            print!("{rendered}");
        }
        Commands::Flags { path, json } => {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Cannot read file: {}", path.display()))?;

            let flags = extract_flags(&content);
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let report = FlagReport {
                file: filename,
                flags,
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", format_flags_text(&report));
            }
        }
    }

    Ok(())
}
