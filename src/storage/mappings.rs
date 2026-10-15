use super::models;
use crate::domain;

impl TryFrom<models::App> for domain::App {
    type Error = anyhow::Error;
    fn try_from(app: models::App) -> anyhow::Result<Self> {
        let build_config: domain::BuildConfig = app
            .build_config
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or_default();
        build_config.validate()?;
        Ok(Self {
            id: app.id,
            provider: app.provider,
            repository: app.repository,
            branch: app.branch,
            domain: app.domain,
            build_config,
            install_command: app.install_command,
            build_command: app.build_command,
            output_directory: app.output_directory,
            spa: app.spa,
            workflow_pr: app.workflow_pr,
            deletion_requested: app.deleted_at.is_some(),
        })
    }
}

impl From<models::Deployment> for domain::Deployment {
    fn from(deployment: models::Deployment) -> Self {
        Self {
            id: deployment.id,
            app_id: deployment.app_id,
            run_id: deployment.run_id,
            attempt: deployment.attempt,
            commit_sha: deployment.commit_sha,
            status: deployment.status,
            error: deployment.error,
            created_at: deployment.created_at,
            finished_at: deployment.finished_at,
        }
    }
}

pub(super) fn job_task(job: &models::Job) -> anyhow::Result<domain::JobTask> {
    match (job.kind.as_str(), job.deployment_id, job.payload.as_deref()) {
        ("deployment", Some(id), None) => Ok(domain::JobTask::Deployment(id)),
        ("email", None, Some(payload)) => {
            let email: crate::mail::Email = serde_json::from_str(payload)?;
            email.validate()?;

            Ok(domain::JobTask::Email(email))
        }
        ("app_config", None, Some(payload)) => Ok(domain::JobTask::SyncApp(payload.parse()?)),
        ("app_remove", None, Some(payload)) => Ok(domain::JobTask::RemoveApp(payload.parse()?)),
        _ => anyhow::bail!("invalid job payload"),
    }
}
