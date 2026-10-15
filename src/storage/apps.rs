use super::{Store, models::App};
use crate::domain::{App as AppInfo, NewApp};

impl Store {
    pub async fn apps(&self) -> anyhow::Result<Vec<AppInfo>> {
        Ok(App::all()
            .exec(&mut self.db.clone())
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub async fn app(&self, id: u64) -> anyhow::Result<AppInfo> {
        App::filter_by_id(id)
            .first()
            .exec(&mut self.db.clone())
            .await?
            .map(Into::into)
            .ok_or_else(|| anyhow::anyhow!("app not found"))
    }

    pub async fn create_app(&self, provider: &str, input: &NewApp) -> anyhow::Result<AppInfo> {
        let app = toasty::create!(App {
            provider,
            repository: &input.repository,
            branch: &input.branch,
            domain: &input.domain,
            install_command: &input.install_command,
            build_command: &input.build_command,
            output_directory: &input.output_directory,
            spa: input.spa,
            workflow_pr: None::<String>,
        })
        .exec(&mut self.db.clone())
        .await?;

        Ok(app.into())
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
