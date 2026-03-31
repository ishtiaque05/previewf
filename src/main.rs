use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use previewf::flags::{extract_flags, format_flags_text, FlagReport};
use previewf::server::{is_markdown, ServerBuilder};
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

        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on
        #[arg(short, long, default_value_t = 4567)]
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

    /// Work with Docker containers
    Docker {
        #[command(subcommand)]
        command: DockerCommands,
    },
}

#[derive(Subcommand)]
enum DockerCommands {
    /// List running Docker containers
    Ls,

    /// Serve files from inside a Docker container
    Serve {
        /// Container name or ID
        container: String,

        /// Path inside the container to serve
        #[arg(default_value = "/")]
        path: String,

        /// Port to listen on
        #[arg(short, long, default_value_t = 4567)]
        port: u16,

        /// Polling interval for file change detection
        #[arg(long, default_value = "2s", value_parser = parse_duration)]
        poll_interval: Duration,
    },
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if let Some(secs) = s.strip_suffix('s') {
        secs.parse::<u64>()
            .map(Duration::from_secs)
            .map_err(|e| e.to_string())
    } else if let Some(ms) = s.strip_suffix("ms") {
        ms.parse::<u64>()
            .map(Duration::from_millis)
            .map_err(|e| e.to_string())
    } else {
        s.parse::<u64>()
            .map(Duration::from_secs)
            .map_err(|_| format!("Invalid duration: {s}. Use e.g. '2s' or '500ms'"))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { path, host, port } => {
            let config = ServerBuilder::new()
                .path(&path)
                .host(host)
                .port(port)
                .live_reload(true)
                .build()
                .context("Failed to configure server")?;

            previewf::server::run(config).await?;
        }
        Commands::View { path } => {
            let name = path.to_string_lossy();
            anyhow::ensure!(
                is_markdown(&name),
                "Not a markdown file: {}",
                path.display()
            );

            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Cannot read file: {}", path.display()))?;

            let rendered = render_terminal(&content);
            print!("{rendered}");
        }
        Commands::Flags { path, json } => {
            let name = path.to_string_lossy();
            anyhow::ensure!(
                is_markdown(&name),
                "Not a markdown file: {}",
                path.display()
            );

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
        Commands::Docker { command } => match command {
            DockerCommands::Ls => {
                previewf::docker::check_docker_available()
                    .await
                    .context("Docker is not available")?;

                let containers = previewf::docker::list_containers()
                    .await
                    .context("Failed to list containers")?;

                if containers.is_empty() {
                    eprintln!("No running containers found.");
                } else {
                    println!(
                        "{:<14} {:<20} {:<25} STATUS",
                        "CONTAINER ID", "NAME", "IMAGE"
                    );
                    for c in &containers {
                        println!(
                            "{:<14} {:<20} {:<25} {}",
                            &c.id[..12.min(c.id.len())],
                            c.name,
                            c.image,
                            c.status
                        );
                    }
                }
            }
            DockerCommands::Serve {
                container,
                path,
                port,
                poll_interval,
            } => {
                previewf::docker::check_docker_available()
                    .await
                    .context("Docker is not available")?;

                previewf::docker::validate_container(&container)
                    .await
                    .with_context(|| format!("Container '{}' is not running", container))?;

                previewf::docker::validate_container_path(&container, &path)
                    .await
                    .with_context(|| {
                        format!("Path '{}' not found in container '{}'", path, container)
                    })?;

                eprintln!(
                    "previewf serving {}:{} on http://localhost:{}",
                    container, path, port
                );
                eprintln!("Polling interval: {}s", poll_interval.as_secs());

                let config = ServerBuilder::new()
                    .path(".")
                    .port(port)
                    .live_reload(false)
                    .build()
                    .context("Failed to configure server")?;

                let source = std::sync::Arc::new(
                    previewf::source::docker::DockerSource::new(container.clone(), path)
                        .context("Failed to create Docker source")?,
                );

                let (reload_tx, _) = tokio::sync::broadcast::channel::<()>(16);

                // Start poll watcher
                let watcher = previewf::docker_watcher::DockerPollWatcher::new(
                    source.clone(),
                    poll_interval,
                    reload_tx.clone(),
                );
                tokio::spawn(async move { watcher.run().await });

                let addr = format!("127.0.0.1:{}", port);
                let listener = tokio::net::TcpListener::bind(&addr)
                    .await
                    .context("Failed to bind address")?;

                let app = previewf::server::create_docker_router(config, source, reload_tx);
                axum::serve(listener, app).await?;
            }
        },
    }

    Ok(())
}
