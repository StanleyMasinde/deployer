use super::models::{App, Deployment, Job, PasswordReset, User};
use anyhow::Context;
use std::time::Duration;

pub async fn connect(url: &str) -> anyhow::Result<toasty::Db> {
    toasty::Db::builder()
        .models(toasty::models!(User, App, Deployment, Job, PasswordReset))
        .max_pool_size(3)
        .pool_wait_timeout(Some(Duration::from_secs(5)))
        .pool_create_timeout(Some(Duration::from_secs(5)))
        .connect(url)
        .await
        .context("failed to connect to the database")
}
