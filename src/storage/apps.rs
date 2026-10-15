use super::{Store, models::App};
use crate::domain::{App as AppInfo, NewApp};

impl Store {
    pub async fn apps(&self) -> anyhow::Result<Vec<AppInfo>> {
        App::all()
            .exec(&mut self.db.clone())
            .await?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub async fn app(&self, id: u64) -> anyhow::Result<AppInfo> {
        self.find_app(id).await
    }

    pub async fn create_app(&self, provider: &str, input: &NewApp) -> anyhow::Result<AppInfo> {
        let build_config = serde_json::to_string(&input.build_config)?;
        let app = toasty::create!(App {
            provider,
            repository: &input.repository,
            branch: &input.branch,
            domain: &input.domain,
            install_command: &input.install_command,
            build_config: Some(build_config),
            build_command: &input.build_command,
            output_directory: &input.output_directory,
            spa: input.spa,
            workflow_pr: None::<String>,
            deleted_at: None::<i64>,
        })
        .exec(&mut self.db.clone())
        .await?;

        app.try_into()
    }

    pub async fn set_pr(&self, id: u64, url: &str) -> anyhow::Result<()> {
        App::filter_by_id(id)
            .update()
            .workflow_pr(Some(url.to_owned()))
            .exec(&mut self.db.clone())
            .await?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum AppError {
    NotFound,
    Deleting,
    Conflict,
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotFound => "Application not found",
            Self::Deleting => "Application deletion is in progress",
            Self::Conflict => "Repository or domain is already connected to an application",
        })
    }
}
impl std::error::Error for AppError {}

impl Store {
    pub async fn find_app(&self, id: u64) -> anyhow::Result<AppInfo> {
        let app = App::filter_by_id(id)
            .first()
            .exec(&mut self.db.clone())
            .await?
            .ok_or(AppError::NotFound)?;
        app.try_into()
    }

    pub async fn update_app(&self, id: u64, input: &NewApp) -> anyhow::Result<AppInfo> {
        let mut db = self.db.clone();
        let mut tx = db.transaction().await?;
        // Lock the row before checking lifecycle state or changing settings.
        toasty::sql::statement("UPDATE apps SET id = id WHERE id = ?")
            .bind(id)
            .exec(&mut tx)
            .await?;
        let current = App::filter_by_id(id)
            .first()
            .exec(&mut tx)
            .await?
            .ok_or(AppError::NotFound)?;
        anyhow::ensure!(current.deleted_at.is_none(), AppError::Deleting);
        let repository_owner = App::filter_by_repository(&input.repository)
            .first()
            .exec(&mut tx)
            .await?;
        let domain_owner = App::filter_by_domain(&input.domain)
            .first()
            .exec(&mut tx)
            .await?;
        anyhow::ensure!(
            !repository_owner.is_some_and(|a| a.id != id)
                && !domain_owner.is_some_and(|a| a.id != id),
            AppError::Conflict
        );
        let build_changed = current.repository != input.repository
            || current.branch != input.branch
            || current.install_command != input.install_command
            || current.build_command != input.build_command
            || current.output_directory != input.output_directory
            || current
                .build_config
                .as_deref()
                .map(serde_json::from_str::<crate::domain::BuildConfig>)
                .transpose()?
                .unwrap_or_default()
                != input.build_config;
        App::filter_by_id(id)
            .update()
            .repository(&input.repository)
            .branch(&input.branch)
            .domain(&input.domain)
            .install_command(&input.install_command)
            .build_command(&input.build_command)
            .output_directory(&input.output_directory)
            .spa(input.spa)
            .build_config(Some(serde_json::to_string(&input.build_config)?))
            .workflow_pr(if build_changed {
                None
            } else {
                current.workflow_pr
            })
            .exec(&mut tx)
            .await?;
        queue_app_task(&mut tx, "app_config", id).await?;
        tx.commit().await?;
        self.find_app(id).await
    }

    pub async fn request_app_deletion(&self, id: u64) -> anyhow::Result<()> {
        let mut db = self.db.clone();
        let mut tx = db.transaction().await?;
        toasty::sql::statement("UPDATE apps SET id = id WHERE id = ?")
            .bind(id)
            .exec(&mut tx)
            .await?;
        let current = App::filter_by_id(id)
            .first()
            .exec(&mut tx)
            .await?
            .ok_or(AppError::NotFound)?;
        if current.deleted_at.is_none() {
            App::filter_by_id(id)
                .update()
                .deleted_at(Some(crate::now()))
                .exec(&mut tx)
                .await?;
        }
        let pending =
            super::models::Job::filter(super::models::Job::fields().kind().eq("app_remove"))
                .exec(&mut tx)
                .await?
                .iter()
                .any(|j| {
                    j.kind == "app_remove"
                        && j.payload.as_deref() == Some(id.to_string().as_str())
                        && matches!(j.status.as_str(), "queued" | "processing")
                });
        if !pending {
            queue_app_task(&mut tx, "app_remove", id).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn purge_app(&self, id: u64) -> anyhow::Result<()> {
        let mut db = self.db.clone();
        let mut tx = db.transaction().await?;
        toasty::sql::statement("UPDATE apps SET id = id WHERE id = ?")
            .bind(id)
            .exec(&mut tx)
            .await?;
        if let Some(app) = App::filter_by_id(id).first().exec(&mut tx).await? {
            anyhow::ensure!(
                app.deleted_at.is_some(),
                "application is not marked for deletion"
            );
        }
        // Only the single worker calls this after removing the Nginx config and files.
        toasty::sql::statement(
            "DELETE j FROM jobs j JOIN deployments d ON j.deployment_id = d.id WHERE d.app_id = ?",
        )
        .bind(id)
        .exec(&mut tx)
        .await?;
        toasty::sql::statement("DELETE FROM deployments WHERE app_id = ?")
            .bind(id)
            .exec(&mut tx)
            .await?;
        toasty::sql::statement("DELETE FROM apps WHERE id = ? AND deleted_at IS NOT NULL")
            .bind(id)
            .exec(&mut tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn queue_app_task(
    tx: &mut toasty::db::Transaction<'_>,
    kind: &str,
    id: u64,
) -> anyhow::Result<()> {
    toasty::sql::statement("INSERT INTO jobs (kind, payload, status, attempts, available_at, created_at) VALUES (?, ?, 'queued', 0, ?, ?)")
        .bind(kind).bind(id.to_string()).bind(crate::now()).bind(crate::now()).exec(tx).await?;
    Ok(())
}
