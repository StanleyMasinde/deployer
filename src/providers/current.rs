use super::{SourceProvider, github::Github};
use crate::{
    config::{ProviderKind, Providers},
    domain::{App, Artifact, BuildCompleted, Repository},
};
use std::{path::Path, sync::Arc};

pub enum CurrentProvider {
    Github(Arc<Github>),
}

pub fn current_provider(config: &Providers) -> anyhow::Result<Arc<CurrentProvider>> {
    let provider = match config.current {
        ProviderKind::Github => CurrentProvider::Github(Github::new(config.github()?.clone())?),
    };

    Ok(Arc::new(provider))
}

impl SourceProvider for CurrentProvider {
    fn name(&self) -> &'static str {
        match self {
            Self::Github(provider) => provider.name(),
        }
    }

    async fn repositories(&self) -> anyhow::Result<Vec<Repository>> {
        match self {
            Self::Github(provider) => provider.repositories().await,
        }
    }

    async fn open_workflow_pr(&self, app: &App) -> anyhow::Result<String> {
        match self {
            Self::Github(provider) => provider.open_workflow_pr(app).await,
        }
    }

    async fn artifact(&self, app: &App, build: &BuildCompleted) -> anyhow::Result<Artifact> {
        match self {
            Self::Github(provider) => provider.artifact(app, build).await,
        }
    }

    async fn download(
        &self,
        app: &App,
        artifact: &Artifact,
        path: &Path,
        limit: u64,
    ) -> anyhow::Result<()> {
        match self {
            Self::Github(provider) => provider.download(app, artifact, path, limit).await,
        }
    }
}
