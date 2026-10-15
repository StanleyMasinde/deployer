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
        Ok(Deployment::filter_by_app_id(app_id)
            .order_by(Deployment::fields().id().desc())
            .limit(100)
            .exec(&mut self.db.clone())
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }
}
