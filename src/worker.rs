use crate::{
    config::Worker,
    domain::{BuildCompleted, JobTask},
    providers::SourceProvider,
    publishing,
    storage::Store,
};
use std::{fs::OpenOptions, sync::Arc, time::Duration};

pub async fn run(
    store: Store,
    provider: Arc<impl SourceProvider>,
    config: Worker,
    mailer: impl crate::mail::EmailSender,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(&config.apps_root)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(config.apps_root.join(".worker.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock)
        .map_err(|_| anyhow::anyhow!("another worker owns this apps directory"))?;
    store.recover().await?;

    let shutdown = crate::shutdown();
    tokio::pin!(shutdown);

    let mut https_check = std::time::Instant::now() - Duration::from_secs(60);
    loop {
        if https_check.elapsed() >= Duration::from_secs(60) {
            if let Err(error) = crate::https::refresh(&store, &config).await {
                tracing::error!(%error, "HTTPS status refresh failed");
            }
            https_check = std::time::Instant::now();
        }
        // Finish the current job before graceful shutdown.
        if let Err(error) = process_one(&store, provider.as_ref(), &config, &mailer).await {
            tracing::error!(%error,"worker operation failed");
        }

        tokio::select! {
            _ = &mut shutdown => break,
            _ = tokio::time::sleep(Duration::from_secs(config.poll_seconds)) => {}
        }
    }

    Ok(())
}

pub async fn process_one(
    store: &Store,
    provider: &impl SourceProvider,
    config: &Worker,
    mailer: &impl crate::mail::EmailSender,
) -> anyhow::Result<()> {
    let Some(job) = store.next().await? else {
        return Ok(());
    };

    let deployment_id = match &job.task {
        JobTask::Email(email) => {
            let result =
                tokio::time::timeout(Duration::from_secs(30), mailer.deliver(job.id, email)).await;
            let failure = match result {
                Ok(Ok(())) => return store.finish(job.id, "completed", None).await,
                Ok(Err(failure)) => failure,
                Err(_) => crate::mail::SendFailure {
                    retryable: true,
                    code: "smtp_timeout",
                },
            };

            tracing::warn!(job = job.id, code = failure.code, "email delivery failed");

            return if failure.retryable && job.attempts < 3 {
                store.retry(job.id, failure.code).await
            } else {
                store.finish(job.id, "failed", Some(failure.code)).await
            };
        }
        JobTask::SyncApp(id) | JobTask::RemoveApp(id) | JobTask::EnableHttps(id) => {
            let remove = matches!(job.task, JobTask::RemoveApp(_));
            let result = async {
                let app = match store.find_app(*id).await {
                    Ok(app) => app,
                    Err(e)
                        if matches!(
                            e.downcast_ref::<crate::storage::AppError>(),
                            Some(crate::storage::AppError::NotFound)
                        ) =>
                    {
                        return Ok(());
                    }
                    Err(e) => return Err(e),
                };
                if remove {
                    anyhow::ensure!(
                        app.deletion_requested,
                        "application is not marked for deletion"
                    );
                    // Apps that have never deployed have no Nginx config or published files.
                    if app.https.enabled
                        || config.apps_root.join(app.id.to_string()).exists()
                        || !store.app_deployments(app.id).await?.is_empty()
                    {
                        publishing::remove_app(config, &app).await?;
                    }
                    if app.https.enabled {
                        crate::https::cleanup(config, &app).await?;
                    }
                    store.purge_app(app.id).await?;
                } else if !app.deletion_requested {
                    if app.https.enabled {
                        crate::https::enable(store, config, &app).await?;
                    } else {
                        publishing::sync_app(config, &app).await?;
                    }
                }
                Ok::<(), anyhow::Error>(())
            }
            .await;
            return match result {
                Ok(()) => store.finish(job.id, "completed", None).await,
                Err(error) if job.attempts < 3 => store.retry(job.id, &error.to_string()).await,
                Err(error) => {
                    store
                        .finish(job.id, "failed", Some(&error.to_string()))
                        .await
                }
            };
        }
        JobTask::Deployment(id) => *id,
    };

    let deployment = store.deployment(deployment_id).await?;

    if store.newer_published(&deployment).await? {
        return store.finish(job.id, "superseded", None).await;
    }

    let result = async {
        let app = store.app(deployment.app_id).await?;
        anyhow::ensure!(!app.deletion_requested, "application is being deleted");
        anyhow::ensure!(app.provider == provider.name(), "provider mismatch");
        let build = BuildCompleted {
            repository: app.repository.clone(),
            branch: app.branch.clone(),
            run_id: deployment.run_id,
            attempt: deployment.attempt,
            commit_sha: deployment.commit_sha.clone(),
        };
        let artifact = provider.artifact(&app, &build).await?;

        let directory = publishing::prepare(config, app.id)?;
        let archive = directory.join(format!("{}.zip", job.id));
        let staging = directory.join(format!("staging-{}", job.id));
        if staging.exists() {
            std::fs::remove_dir_all(&staging)?;
        }

        let result = async {
            provider
                .download(&app, &artifact, &archive, config.max_artifact_bytes)
                .await?;

            let (source, target, limit) =
                (archive.clone(), staging.clone(), config.max_extracted_bytes);
            tokio::task::spawn_blocking(move || {
                publishing::archive::extract(&source, &target, limit)
            })
            .await??;
            publishing::publish(config, &app, &staging).await
        }
        .await;
        if archive.exists() {
            std::fs::remove_file(&archive)?;
        }

        if staging.exists() {
            std::fs::remove_dir_all(&staging)?;
        }
        result
    }
    .await;
    match result {
        Ok(()) => {
            tracing::info!(deployment = deployment.id, "app published");
            store.finish(job.id, "live", None).await?;
        }
        Err(error) => {
            tracing::error!(deployment=deployment.id,%error,"deployment failed");
            let message = error.to_string().chars().take(1000).collect::<String>();
            if job.attempts < 3 {
                store.retry(job.id, &message).await?;
            } else {
                store.finish(job.id, "failed", Some(&message)).await?;
            }
        }
    }

    Ok(())
}
