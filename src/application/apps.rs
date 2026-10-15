use crate::{
    domain::{App, NewApp},
    providers::SourceProvider,
    storage::Store,
};

pub async fn add(
    store: &Store,
    provider: &impl SourceProvider,
    input: NewApp,
) -> anyhow::Result<App> {
    input.validate()?;
    anyhow::ensure!(
        provider
            .repositories()
            .await?
            .iter()
            .any(|r| r.full_name.eq_ignore_ascii_case(&input.repository)),
        "repository is not accessible to this provider installation"
    );
    let mut input = input;
    input.repository = input.repository.to_ascii_lowercase();
    let app = store.create_app(provider.name(), &input).await?;
    // Persist first: a failed GitHub request can be retried using the existing app.
    create_pr(store, provider, app.id).await
}

pub async fn create_pr(
    store: &Store,
    provider: &impl SourceProvider,
    id: u64,
) -> anyhow::Result<App> {
    let mut app = store.find_app(id).await?;
    anyhow::ensure!(!app.deletion_requested, crate::storage::AppError::Deleting);
    anyhow::ensure!(app.provider == provider.name(), "provider mismatch");
    let url = provider.open_workflow_pr(&app).await?;
    store.set_pr(id, &url).await?;
    app.workflow_pr = Some(url);
    Ok(app)
}

pub async fn update(
    store: &Store,
    provider: &impl SourceProvider,
    id: u64,
    mut input: NewApp,
) -> anyhow::Result<App> {
    input.validate()?;
    let current = store.find_app(id).await?;
    anyhow::ensure!(
        !current.deletion_requested,
        crate::storage::AppError::Deleting
    );
    anyhow::ensure!(current.provider == provider.name(), "provider mismatch");
    if !input.repository.eq_ignore_ascii_case(&current.repository) {
        anyhow::ensure!(
            provider
                .repositories()
                .await?
                .iter()
                .any(|r| r.full_name.eq_ignore_ascii_case(&input.repository)),
            "repository is not accessible to this provider installation"
        );
    }
    input.repository = input.repository.to_ascii_lowercase();
    store.update_app(id, &input).await
}
