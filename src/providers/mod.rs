mod current;
pub mod github;

pub use current::{CurrentProvider, current_provider};

use crate::domain::{App, Artifact, BuildCompleted, Repository};
use std::{future::Future, path::Path};

pub trait SourceProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn repositories(&self) -> impl Future<Output = anyhow::Result<Vec<Repository>>> + Send;
    fn open_workflow_pr(&self, app: &App) -> impl Future<Output = anyhow::Result<String>> + Send;
    fn artifact(
        &self,
        app: &App,
        build: &BuildCompleted,
    ) -> impl Future<Output = anyhow::Result<Artifact>> + Send;
    fn download(
        &self,
        app: &App,
        artifact: &Artifact,
        path: &Path,
        limit: u64,
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
}
