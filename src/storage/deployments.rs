use super::{Store, models::Deployment};
use crate::domain::Deployment as DeploymentInfo;

impl Store {
    pub async fn deployments(&self) -> anyhow::Result<Vec<DeploymentInfo>> {
        Ok(Deployment::all()
            .order_by(Deployment::fields().id().desc())
            .limit(100)
            .exec(&mut self.db.clone())
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub async fn deployment(&self, id: u64) -> anyhow::Result<DeploymentInfo> {
        Deployment::filter_by_id(id)
            .first()
            .exec(&mut self.db.clone())
            .await?
            .map(Into::into)
            .ok_or_else(|| anyhow::anyhow!("deployment not found"))
    }

    pub async fn newer_published(&self, deployment: &DeploymentInfo) -> anyhow::Result<bool> {
        let fields = Deployment::fields();
        let newer = fields.run_id().gt(deployment.run_id).or(fields
            .run_id()
            .eq(deployment.run_id)
            .and(fields.attempt().gt(deployment.attempt)));
        let row = Deployment::filter(
            fields
                .app_id()
                .eq(deployment.app_id)
                .and(fields.status().eq("live"))
                .and(newer),
        )
        .first()
        .exec(&mut self.db.clone())
        .await?;

        Ok(row.is_some())
    }
}

impl Store {
    pub async fn app_deployments(&self, app_id: u64) -> anyhow::Result<Vec<DeploymentInfo>> {
        self.app_deployments_before(app_id, None).await
    }

    pub async fn app_deployments_before(
        &self,
        app_id: u64,
        before: Option<u64>,
    ) -> anyhow::Result<Vec<DeploymentInfo>> {
        let fields = Deployment::fields();
        Ok(Deployment::filter(
            fields
                .app_id()
                .eq(app_id)
                .and(fields.id().lt(before.unwrap_or(u64::MAX))),
        )
        .order_by(Deployment::fields().id().desc())
        .limit(100)
        .exec(&mut self.db.clone())
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
    }
}

impl Store {
    pub async fn deployment_optional(&self, id: u64) -> anyhow::Result<Option<DeploymentInfo>> {
        Ok(Deployment::filter_by_id(id)
            .first()
            .exec(&mut self.db.clone())
            .await?
            .map(Into::into))
    }
}

impl Store {
    pub async fn record_workflow_status(
        &self,
        build: &crate::domain::BuildCompleted,
        status: &str,
        error: Option<&str>,
    ) -> anyhow::Result<()> {
        for app in self.apps().await? {
            if app.provider != "github"
                || app.deletion_requested
                || !app.repository.eq_ignore_ascii_case(&build.repository)
                || app.branch != build.branch
            {
                continue;
            }
            let key = format!("github:{}:{}:{}", app.id, build.run_id, build.attempt);
            toasty::sql::statement("INSERT INTO deployments (event_key, app_id, run_id, attempt, commit_sha, status, error, created_at, finished_at) VALUES (?, ?, ?, ?, ?, ?, NULLIF(?, ''), ?, NULLIF(?, 0)) ON DUPLICATE KEY UPDATE error = IF(status = 'building', VALUES(error), error), finished_at = IF(status = 'building', VALUES(finished_at), finished_at), status = IF(status = 'building', VALUES(status), status)")
                .bind(&key).bind(app.id).bind(build.run_id).bind(build.attempt).bind(&build.commit_sha).bind(status).bind(error.unwrap_or_default()).bind(crate::now()).bind(if status == "failed" { crate::now() } else { 0 })
                .exec(&mut self.db.clone()).await?;
        }
        Ok(())
    }
}
