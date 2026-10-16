mod apps;
pub use apps::AppError;
pub mod db;
mod deployments;
mod jobs;
mod mappings;
pub mod models;
mod users;

#[derive(Clone)]
pub struct Store {
    db: toasty::Db,
    pub changes: std::sync::Arc<tokio::sync::Notify>,
}

impl Store {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let db = db::connect(url).await?;

        Ok(Self {
            db,
            changes: std::sync::Arc::new(tokio::sync::Notify::new()),
        })
    }

    pub fn notify_changes(&self) {
        self.changes.notify_waiters();
    }

    pub fn database(&self) -> toasty::Db {
        self.db.clone()
    }
}
