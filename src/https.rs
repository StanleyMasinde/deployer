use crate::{config::Worker, domain::App, storage::Store};
use anyhow::{Context, ensure};
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Certificate {
    expires_at: i64,
    renewal_failed: bool,
}

async fn command(config: &Worker, app: &App, action: &str) -> anyhow::Result<Vec<u8>> {
    crate::domain::validate_domain(&app.domain)?;
    let output = tokio::time::timeout(
        Duration::from_secs(if action == "status" { 10 } else { 480 }),
        tokio::process::Command::new("sudo")
            .arg("-n")
            .arg(&config.https_helper)
            .arg(action)
            .arg(app.id.to_string())
            .arg(&app.domain)
            .arg(if app.spa { "spa" } else { "static" })
            .kill_on_drop(true)
            .output(),
    )
    .await
    .context("HTTPS operation timed out")??;
    if !output.status.success() {
        tracing::error!(app = app.id, stderr = %String::from_utf8_lossy(&output.stderr), "HTTPS helper failed");
        if output.status.code() == Some(3) {
            anyhow::bail!(
                "This application has TLS configured outside Deployer. Migrate its Nginx configuration before enabling managed HTTPS."
            );
        }
        if output.status.code() == Some(4) {
            anyhow::bail!("HTTPS tooling is missing. Rerun the Deployer installer on the VPS.");
        }
        anyhow::bail!(
            "HTTPS operation failed. Check DNS, port 80, and the worker logs, then retry."
        );
    }
    Ok(output.stdout)
}

pub async fn enable(store: &Store, config: &Worker, app: &App) -> anyhow::Result<()> {
    let result = async {
        let certificate: Certificate =
            serde_json::from_slice(&command(config, app, "issue").await?)
                .context("read certificate status")?;
        ensure!(
            certificate.expires_at > crate::now(),
            "certificate has expired"
        );
        store
            .record_https(app.id, &app.domain, certificate.expires_at, None)
            .await
    }
    .await;
    if let Err(ref error) = result {
        store
            .https_failed(app.id, &app.domain, &error.to_string())
            .await?;
    }
    result
}

pub async fn cleanup(config: &Worker, app: &App) -> anyhow::Result<()> {
    command(config, app, "cleanup").await?;
    Ok(())
}

pub async fn refresh(store: &Store, config: &Worker) -> anyhow::Result<()> {
    for app in store.apps().await? {
        if !app.https.enabled || app.deletion_requested || app.https.expires_at.is_none() {
            continue;
        }
        match command(config, &app, "status")
            .await
            .and_then(|bytes| Ok(serde_json::from_slice::<Certificate>(&bytes)?))
        {
            Ok(cert) => store
                .record_https(
                    app.id,
                    &app.domain,
                    cert.expires_at,
                    if cert.renewal_failed {
                        Some(
                            "Certificate renewal failed. Check the renewal service logs and retry.",
                        )
                    } else {
                        None
                    },
                )
                .await?,
            Err(error) => {
                store
                    .https_failed(app.id, &app.domain, &error.to_string())
                    .await?;
            }
        }
    }
    Ok(())
}
