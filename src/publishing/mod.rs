pub mod archive;

use crate::{config::Worker, domain::App};
use anyhow::{Context, ensure};
use std::{
    future::Future,
    path::{Path, PathBuf},
};

pub trait Activator {
    fn activate(&self, app: &App) -> impl Future<Output = anyhow::Result<()>> + Send;
}

struct Nginx<'a>(&'a Worker);

impl Activator for Nginx<'_> {
    async fn activate(&self, app: &App) -> anyhow::Result<()> {
        let command = tokio::process::Command::new("sudo")
            .arg("-n")
            .arg(&self.0.nginx_helper)
            .arg(app.id.to_string())
            .arg(&app.domain)
            .arg(if app.spa { "spa" } else { "static" })
            .kill_on_drop(true)
            .output();

        let output = tokio::time::timeout(std::time::Duration::from_secs(30), command)
            .await
            .context("Nginx activation timed out")??;

        if !output.status.success() {
            tracing::error!(stderr = %String::from_utf8_lossy(&output.stderr), "Nginx helper failed");
            anyhow::bail!("Nginx activation failed; check the worker and Nginx service logs");
        }

        Ok(())
    }
}

pub async fn publish(config: &Worker, app: &App, staging: &Path) -> anyhow::Result<()> {
    publish_with(config, app, staging, &Nginx(config)).await
}

pub async fn publish_with(
    config: &Worker,
    app: &App,
    staging: &Path,
    activator: &impl Activator,
) -> anyhow::Result<()> {
    crate::domain::validate_domain(&app.domain)?;

    let directory = config.apps_root.join(app.id.to_string());
    let current = directory.join("current");
    let previous = std::fs::read_link(&current).ok();
    let mut nonce = [0_u8; 8];
    rand::fill(&mut nonce);
    let name = format!("content-{}", hex::encode(nonce));
    let content = directory.join(&name);

    ensure!(
        staging.join("index.html").is_file(),
        "app is missing index.html"
    );
    std::fs::rename(staging, &content)?;

    // A single rename switches the document root without a missing-directory gap.
    swap_link(&directory, Path::new(&name))?;

    if let Err(error) = activator.activate(app).await {
        if let Some(previous) = &previous {
            swap_link(&directory, previous)?;
        } else {
            std::fs::remove_file(&current)?;
        }

        std::fs::remove_dir_all(content)?;
        return Err(error.context("previous app restored"));
    }

    if let Some(previous) = previous
        && previous.components().count() == 1
        && previous.to_string_lossy().starts_with("content-")
    {
        std::fs::remove_dir_all(directory.join(previous))?;
    }

    Ok(())
}

fn swap_link(directory: &Path, target: &Path) -> anyhow::Result<()> {
    let next = directory.join(".current-next");

    if next.symlink_metadata().is_ok() {
        std::fs::remove_file(&next)?;
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(target, &next)?;

    #[cfg(not(unix))]
    anyhow::bail!("app publishing requires Unix");

    std::fs::rename(next, directory.join("current"))?;
    Ok(())
}

pub fn prepare(config: &Worker, app_id: u64) -> anyhow::Result<PathBuf> {
    let directory = config.apps_root.join(app_id.to_string());
    std::fs::create_dir_all(&directory).context("create app directory")?;
    let current = std::fs::read_link(directory.join("current")).ok();

    // Recover temporary files left by a killed worker. Keep only the live content.
    for entry in std::fs::read_dir(&directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if entry.file_type()?.is_dir()
            && (name.starts_with("staging-") || name.starts_with("content-"))
            && current.as_deref() != Some(Path::new(name.as_ref()))
        {
            std::fs::remove_dir_all(entry.path())?;
        }

        if entry.file_type()?.is_file() && name.ends_with(".zip") {
            std::fs::remove_file(entry.path())?;
        }
    }

    Ok(directory)
}
