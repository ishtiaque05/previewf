use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Command;

use super::{DirEntry, EntryType, FileMeta, FileSource};
use crate::docker::validate_container_name;
use crate::PreviewError;

/// Timeout for all `docker exec` subprocess calls.
const DOCKER_EXEC_TIMEOUT: Duration = Duration::from_secs(30);

/// File source backed by a Docker container's filesystem.
///
/// All operations shell out via `docker exec` with separate arguments
/// (no shell interpolation) to prevent injection.
pub struct DockerSource {
    container: String,
    base_path: String,
}

impl DockerSource {
    pub fn new(container: String, base_path: String) -> Result<Self, PreviewError> {
        validate_container_name(&container)?;
        let base_path = normalize_container_path(&base_path)?;
        Ok(Self {
            container,
            base_path,
        })
    }

    pub fn container(&self) -> &str {
        &self.container
    }

    /// Resolve a relative path against the base, preventing traversal.
    fn resolve(&self, path: &str) -> Result<String, PreviewError> {
        let full = if path.is_empty() {
            self.base_path.clone()
        } else {
            let normalized = normalize_container_path(path)?;
            if self.base_path == "/" {
                normalized
            } else {
                format!("{}{}", self.base_path, normalized)
            }
        };

        if !full.starts_with(&self.base_path) {
            return Err(PreviewError::ContainerPathNotFound {
                container: self.container.clone(),
                path: path.to_string(),
            });
        }

        Ok(full)
    }
}

/// Normalize a container path: ensure leading slash, collapse double slashes,
/// remove "." segments, reject ".." segments with an error.
fn normalize_container_path(path: &str) -> Result<String, PreviewError> {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return Ok("/".to_string());
    }
    let cleaned: Vec<&str> = trimmed
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if cleaned.contains(&"..") {
        return Err(PreviewError::ContainerPathNotFound {
            container: String::new(),
            path: path.to_string(),
        });
    }
    Ok(format!("/{}", cleaned.join("/")))
}

/// Run a docker exec command with a timeout.
async fn docker_exec_output(cmd: &mut Command) -> Result<std::process::Output, PreviewError> {
    tokio::time::timeout(DOCKER_EXEC_TIMEOUT, cmd.output())
        .await
        .map_err(|_| PreviewError::DockerExec("docker exec timed out".to_string()))?
        .map_err(|e| PreviewError::DockerExec(e.to_string()))
}

#[async_trait]
impl FileSource for DockerSource {
    async fn read_file(&self, path: &str) -> Result<String, PreviewError> {
        let full = self.resolve(path)?;
        let output = docker_exec_output(Command::new("docker").args([
            "exec",
            &self.container,
            "cat",
            &full,
        ]))
        .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(PreviewError::ContainerPathNotFound {
                container: self.container.clone(),
                path: path.to_string(),
            })
        }
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, PreviewError> {
        let full = self.resolve(path)?;
        let output = docker_exec_output(Command::new("docker").args([
            "exec",
            &self.container,
            "ls",
            "-1F",
            &full,
        ]))
        .await?;

        if !output.status.success() {
            return Err(PreviewError::ContainerPathNotFound {
                container: self.container.clone(),
                path: path.to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries: Vec<DirEntry> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let name = line.trim();
                if name.starts_with('.') {
                    return None;
                }
                if let Some(dir_name) = name.strip_suffix('/') {
                    Some(DirEntry {
                        name: dir_name.to_string(),
                        entry_type: EntryType::Directory,
                    })
                } else {
                    let clean = name.trim_end_matches(['*', '@', '|', '=']);
                    Some(DirEntry {
                        name: clean.to_string(),
                        entry_type: EntryType::File,
                    })
                }
            })
            .collect();

        entries.sort_by_key(|e| e.name.to_lowercase());
        Ok(entries)
    }

    async fn stat(&self, path: &str) -> Result<FileMeta, PreviewError> {
        let full = self.resolve(path)?;
        let output = docker_exec_output(Command::new("docker").args([
            "exec",
            &self.container,
            "stat",
            "-c",
            "%Y %s %F",
            &full,
        ]))
        .await?;

        if !output.status.success() {
            return Err(PreviewError::ContainerPathNotFound {
                container: self.container.clone(),
                path: path.to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = stdout.trim().splitn(3, ' ').collect();
        if parts.len() < 3 {
            return Err(PreviewError::DockerExec(format!(
                "Unexpected stat output: {stdout}"
            )));
        }

        let mtime = parts[0].parse::<u64>().unwrap_or(0);
        let size = parts[1].parse::<u64>().unwrap_or(0);
        let is_dir = parts[2].contains("directory");

        Ok(FileMeta {
            mtime,
            size,
            is_dir,
        })
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<(), PreviewError> {
        let full = self.resolve(path)?;
        // Use tee with direct args — no shell interpolation, no injection surface.
        let mut child = tokio::process::Command::new("docker")
            .args(["exec", "-i", &self.container, "tee", &full])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

        let Some(mut stdin) = child.stdin.take() else {
            return Err(PreviewError::DockerExec(
                "Failed to open stdin pipe for docker exec".to_string(),
            ));
        };

        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(content.as_bytes())
            .await
            .map_err(|e| PreviewError::DockerExec(e.to_string()))?;
        drop(stdin); // Close pipe so tee receives EOF

        let status = tokio::time::timeout(DOCKER_EXEC_TIMEOUT, child.wait())
            .await
            .map_err(|_| PreviewError::DockerExec("docker exec timed out".to_string()))?
            .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

        if status.success() {
            Ok(())
        } else {
            Err(PreviewError::DockerExec(format!(
                "Failed to write {full} in container {}",
                self.container
            )))
        }
    }

    async fn is_file(&self, path: &str) -> bool {
        let Ok(full) = self.resolve(path) else {
            return false;
        };
        docker_exec_output(Command::new("docker").args([
            "exec",
            &self.container,
            "test",
            "-f",
            &full,
        ]))
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
    }

    async fn is_dir(&self, path: &str) -> bool {
        let Ok(full) = self.resolve(path) else {
            return false;
        };
        docker_exec_output(Command::new("docker").args([
            "exec",
            &self.container,
            "test",
            "-d",
            &full,
        ]))
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
    }

    fn display_root(&self) -> String {
        format!("{}:{}", self.container, self.base_path)
    }
}
