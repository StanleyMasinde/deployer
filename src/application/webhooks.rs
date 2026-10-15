use crate::{domain::BuildCompleted, storage::Store};

pub async fn completed(store: &Store, provider: &str, build: BuildCompleted) -> anyhow::Result<()> {
    for app in store.apps().await? {
        if app.provider == provider
            && app.repository.eq_ignore_ascii_case(&build.repository)
            && app.branch == build.branch
        {
            store.enqueue(&app, &build).await?;
        }
    }

    Ok(())
}
