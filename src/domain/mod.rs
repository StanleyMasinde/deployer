mod build;
pub use build::{BuildConfig, DeploymentType, Ecosystem, Framework, PackageManager};

use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub id: u64,
    pub provider: String,
    pub repository: String,
    pub branch: String,
    pub domain: String,
    pub build_config: BuildConfig,
    pub install_command: String,
    pub build_command: String,
    pub output_directory: String,
    pub spa: bool,
    pub workflow_pr: Option<String>,
    pub deletion_requested: bool,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NewApp {
    pub repository: String,
    pub branch: String,
    pub domain: String,
    #[serde(default)]
    pub build_config: BuildConfig,
    #[serde(default)]
    pub install_command: String,
    #[serde(default)]
    pub build_command: String,
    pub output_directory: String,
    #[serde(default)]
    pub spa: bool,
}

impl NewApp {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_repository(&self.repository)?;
        validate_domain(&self.domain)?;
        ensure!(
            !self.branch.is_empty()
                && self.branch.len() <= 191
                && self
                    .branch
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"/_.-".contains(&c))
                && !self.branch.contains(".."),
            "invalid branch"
        );
        self.build_config.validate()?;
        if self.build_config.ecosystem == Ecosystem::None {
            ensure!(
                self.install_command.is_empty() && self.build_command.is_empty(),
                "plain static sites must not have install or build commands"
            );
        }
        for command in [&self.install_command, &self.build_command] {
            ensure!(
                (self.build_config.ecosystem == Ecosystem::None || !command.trim().is_empty())
                    && command.len() <= 2000
                    && !command.contains('\0'),
                "build commands must contain 1–2000 characters"
            );
        }

        ensure!(
            !self.output_directory.is_empty()
                && self.output_directory.len() <= 191
                && !self.output_directory.starts_with('/')
                && (self.output_directory == "."
                    || self.output_directory.split('/').all(|p| !p.is_empty()
                        && p != ".."
                        && p != "."
                        && p.bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c)))),
            "output directory must be a relative path without traversal"
        );
        Ok(())
    }
}

pub fn validate_repository(repository: &str) -> anyhow::Result<()> {
    let parts: Vec<_> = repository.split('/').collect();
    ensure!(
        parts.len() == 2
            && parts.iter().all(|p| !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.len() <= 90
                && p.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))),
        "repository must be OWNER/REPO"
    );
    Ok(())
}

pub fn validate_domain(domain: &str) -> anyhow::Result<()> {
    ensure!(
        domain.len() <= 253 && domain.contains('.'),
        "enter a fully qualified domain"
    );
    for label in domain.split('.') {
        ensure!(
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
            "invalid domain; use lowercase ASCII or punycode"
        );
    }

    if domain.parse::<std::net::IpAddr>().is_ok() {
        bail!("use a domain, not an IP address");
    }

    Ok(())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deployment {
    pub id: u64,
    pub app_id: u64,
    pub run_id: u64,
    pub attempt: u32,
    pub commit_sha: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

pub struct Job {
    pub id: u64,
    pub task: JobTask,
    pub attempts: u32,
}

pub enum JobTask {
    Deployment(u64),
    Email(crate::mail::Email),
    SyncApp(u64),
    RemoveApp(u64),
}

impl Job {
    pub fn deployment_id(&self) -> Option<u64> {
        match &self.task {
            JobTask::Deployment(id) => Some(*id),
            JobTask::Email(_) | JobTask::SyncApp(_) | JobTask::RemoveApp(_) => None,
        }
    }
}

#[derive(Clone)]
pub struct BuildCompleted {
    pub repository: String,
    pub branch: String,
    pub run_id: u64,
    pub attempt: u32,
    pub commit_sha: String,
}

#[derive(Clone)]
pub struct Artifact {
    pub id: u64,
    pub digest: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub full_name: String,
    pub default_branch: String,
}
