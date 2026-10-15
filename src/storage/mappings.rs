use super::models;
use crate::domain;

impl From<models::App> for domain::App {
    fn from(app: models::App) -> Self {
        Self {
            id: app.id,
            provider: app.provider,
            repository: app.repository,
            branch: app.branch,
            domain: app.domain,
            install_command: app.install_command,
            build_command: app.build_command,
            output_directory: app.output_directory,
            spa: app.spa,
            workflow_pr: app.workflow_pr,
        }
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
        _ => anyhow::bail!("invalid job payload"),
    }
}
