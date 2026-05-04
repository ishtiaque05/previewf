use serde::Serialize;
use tokio::process::Command;

use crate::PreviewError;

#[derive(Debug, Clone, Serialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub workdir: String,
}

pub fn parse_container_list(output: &str) -> Vec<ContainerInfo> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                Some(ContainerInfo {
                    id: parts[0].to_string(),
                    name: parts[1].to_string(),
                    image: parts[2].to_string(),
                    status: parts[3].to_string(),
                    workdir: "/".to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Get the configured `WorkingDir` for a container via `docker inspect`.
pub async fn get_container_workdir(name: &str) -> String {
    let output = Command::new("docker")
        .args(["inspect", "--format", "{{.Config.WorkingDir}}", name])
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => {
            let dir = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if dir.is_empty() {
                "/".to_string()
            } else {
                dir
            }
        }
        _ => "/".to_string(),
    }
}

pub fn validate_container_name(name: &str) -> Result<(), PreviewError> {
    if name.is_empty() {
        return Err(PreviewError::ContainerNotFound(String::new()));
    }
    let valid = name.chars().enumerate().all(|(i, c)| {
        if i == 0 {
            c.is_ascii_alphanumeric()
        } else {
            c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-'
        }
    });
    if valid {
        Ok(())
    } else {
        Err(PreviewError::ContainerNotFound(format!(
            "Invalid container name: {name}"
        )))
    }
}

pub async fn check_docker_available() -> Result<String, PreviewError> {
    let output = Command::new("docker")
        .args(["version", "--format", "{{.Client.Version}}"])
        .output()
        .await
        .map_err(|_| {
            PreviewError::DockerNotAvailable("Docker CLI not found. Is Docker installed?".into())
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(PreviewError::DockerNotAvailable(
            "Docker CLI not responding".into(),
        ))
    }
}

pub async fn list_containers() -> Result<Vec<ContainerInfo>, PreviewError> {
    let output = Command::new("docker")
        .args([
            "ps",
            "--format",
            "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}",
        ])
        .output()
        .await
        .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

    if !output.status.success() {
        return Err(PreviewError::DockerExec(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut containers = parse_container_list(&stdout);

    // Enrich each container with its WorkingDir from docker inspect
    for c in &mut containers {
        c.workdir = get_container_workdir(&c.name).await;
    }

    Ok(containers)
}

pub async fn validate_container(name: &str) -> Result<(), PreviewError> {
    validate_container_name(name)?;

    let output = Command::new("docker")
        .args(["inspect", "--format", "{{.State.Running}}", name])
        .output()
        .await
        .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

    if !output.status.success() {
        return Err(PreviewError::ContainerNotFound(name.to_string()));
    }

    let running = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if running == "true" {
        Ok(())
    } else {
        Err(PreviewError::ContainerNotRunning(name.to_string()))
    }
}

pub async fn validate_container_path(container: &str, path: &str) -> Result<(), PreviewError> {
    let output = Command::new("docker")
        .args(["exec", container, "test", "-e", path])
        .output()
        .await
        .map_err(|e| PreviewError::DockerExec(e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(PreviewError::ContainerPathNotFound {
            container: container.to_string(),
            path: path.to_string(),
        })
    }
}
