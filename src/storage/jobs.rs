use super::{
    Store,
    models::{Deployment, Job},
};
use crate::domain::{App, BuildCompleted, Job as JobInfo};

impl Store {
    pub async fn enqueue(&self, app: &App, build: &BuildCompleted) -> anyhow::Result<()> {
        let key = format!(
            "{}:{}:{}:{}",
            app.provider, app.id, build.run_id, build.attempt
        );
        let mut db = self.db.clone();
        let mut transaction = db.transaction().await?;

        toasty::sql::statement("UPDATE apps SET id = id WHERE id = ?")
            .bind(app.id)
            .exec(&mut transaction)
            .await?;
        let current = super::models::App::filter_by_id(app.id)
            .first()
            .exec(&mut transaction)
            .await?;
        let Some(current) = current else {
            return Ok(());
        };
        if current.deleted_at.is_some()
            || current.branch != build.branch
            || !current.repository.eq_ignore_ascii_case(&build.repository)
        {
            return Ok(());
        }
        // Both rows are committed together, including concurrent webhook redeliveries.
        toasty::sql::statement(
            "INSERT INTO deployments
                (event_key, app_id, run_id, attempt, commit_sha, status, created_at)
             VALUES (?, ?, ?, ?, ?, 'queued', ?)
             ON DUPLICATE KEY UPDATE event_key = event_key, status = IF(status = 'building', 'queued', status)",
        )
        .bind(&key)
        .bind(app.id)
        .bind(build.run_id)
        .bind(build.attempt)
        .bind(&build.commit_sha)
        .bind(crate::now())
        .exec(&mut transaction)
        .await?;

        let deployment = Deployment::filter_by_event_key(&key)
            .first()
            .exec(&mut transaction)
            .await?
            .ok_or_else(|| anyhow::anyhow!("queued deployment missing"))?;

        toasty::sql::statement(
            "INSERT INTO jobs (deployment_id, kind, status, attempts, available_at, created_at)
             VALUES (?, 'deployment', 'queued', 0, ?, ?)
             ON DUPLICATE KEY UPDATE deployment_id = deployment_id",
        )
        .bind(deployment.id)
        .bind(crate::now())
        .bind(crate::now())
        .exec(&mut transaction)
        .await?;

        transaction.commit().await?;

        Ok(())
    }

    pub async fn recover(&self) -> anyhow::Result<()> {
        // Only called after obtaining the exclusive worker process lock.
        let mut db = self.db.clone();
        let mut transaction = db.transaction().await?;

        toasty::sql::statement("UPDATE apps a JOIN jobs j ON j.payload = CAST(a.id AS CHAR) SET a.https_error = 'Certificate operation interrupted on final attempt. Retry HTTPS.' WHERE a.https_enabled = 1 AND j.kind IN ('https', 'app_config') AND j.status = 'processing' AND j.attempts >= 3")
            .exec(&mut transaction).await?;

        toasty::sql::statement(
            "UPDATE deployments d JOIN jobs j ON j.deployment_id = d.id
             SET d.status = 'failed', d.error = 'Worker interrupted on final attempt', d.finished_at = ?
             WHERE j.status = 'processing' AND j.attempts >= 3",
        ).bind(crate::now()).exec(&mut transaction).await?;

        toasty::sql::statement(
            "UPDATE jobs SET status = 'failed', error = 'attempts_exhausted', payload = NULL, finished_at = ?
             WHERE status = 'processing' AND attempts >= 3",
        )
        .bind(crate::now())
        .exec(&mut transaction)
        .await?;

        toasty::sql::statement(
            "UPDATE deployments d JOIN jobs j ON j.deployment_id = d.id
             SET d.status = 'queued' WHERE j.status = 'processing'",
        )
        .exec(&mut transaction)
        .await?;

        toasty::sql::statement(
            "UPDATE jobs SET status = 'queued', available_at = ? WHERE status = 'processing'",
        )
        .bind(crate::now())
        .exec(&mut transaction)
        .await?;

        transaction.commit().await?;

        Ok(())
    }

    pub async fn next(&self) -> anyhow::Result<Option<JobInfo>> {
        let now = crate::now();
        let fields = Job::fields();
        let row = Job::filter(
            fields
                .status()
                .eq("queued")
                .or(fields.status().eq("processing"))
                .and(fields.available_at().le(now)),
        )
        .order_by(fields.id().asc())
        .first()
        .exec(&mut self.db.clone())
        .await?;

        let Some(job) = row else {
            return Ok(None);
        };

        if job.attempts >= 3 {
            self.finish(
                job.id,
                "failed",
                Some("Worker interrupted on final attempt"),
            )
            .await?;
            return Ok(None);
        }

        let mut db = self.db.clone();
        let mut transaction = db.transaction().await?;
        let claimed = toasty::sql::statement(
            "UPDATE jobs SET status = 'processing', attempts = attempts + 1, available_at = ?
             WHERE id = ? AND status IN ('queued', 'processing') AND available_at <= ?",
        )
        .bind(now + 600)
        .bind(job.id)
        .bind(now)
        .exec(&mut transaction)
        .await?;

        if claimed == 0 {
            return Ok(None);
        }

        if let Some(deployment_id) = job.deployment_id {
            Deployment::filter_by_id(deployment_id)
                .update()
                .status("deploying")
                .exec(&mut transaction)
                .await?;
        }

        transaction.commit().await?;

        let task = match super::mappings::job_task(&job) {
            Ok(task) => task,
            Err(_) => {
                self.finish(job.id, "failed", Some("invalid_job_payload"))
                    .await?;
                return Ok(None);
            }
        };

        Ok(Some(JobInfo {
            id: job.id,
            task,
            attempts: job.attempts + 1,
        }))
    }

    pub async fn finish(&self, id: u64, status: &str, error: Option<&str>) -> anyhow::Result<()> {
        let mut db = self.db.clone();
        let mut transaction = db.transaction().await?;
        let job = Job::filter_by_id(id)
            .first()
            .exec(&mut transaction)
            .await?
            .ok_or_else(|| anyhow::anyhow!("job not found"))?;

        Job::filter_by_id(id)
            .update()
            .status(if status == "failed" {
                "failed"
            } else {
                "completed"
            })
            .error(error.map(str::to_owned))
            .payload(None::<String>)
            .finished_at(Some(crate::now()))
            .exec(&mut transaction)
            .await?;

        if let Some(deployment_id) = job.deployment_id {
            Deployment::filter_by_id(deployment_id)
                .update()
                .status(status)
                .error(error.map(str::to_owned))
                .finished_at(Some(crate::now()))
                .exec(&mut transaction)
                .await?;
        }

        transaction.commit().await?;

        Ok(())
    }

    pub async fn retry(&self, id: u64, error: &str) -> anyhow::Result<()> {
        let mut db = self.db.clone();
        let mut transaction = db.transaction().await?;
        let job = Job::filter_by_id(id)
            .first()
            .exec(&mut transaction)
            .await?
            .ok_or_else(|| anyhow::anyhow!("job not found"))?;

        Job::filter_by_id(id)
            .update()
            .status("queued")
            .available_at(crate::now() + 30)
            .error(Some(error.to_owned()))
            .exec(&mut transaction)
            .await?;

        if let Some(deployment_id) = job.deployment_id {
            Deployment::filter_by_id(deployment_id)
                .update()
                .status("queued")
                .error(Some(error.to_owned()))
                .exec(&mut transaction)
                .await?;
        }

        transaction.commit().await?;

        Ok(())
    }
}

pub(super) async fn insert_email(
    transaction: &mut toasty::db::Transaction<'_>,
    email: &crate::mail::Email,
) -> anyhow::Result<u64> {
    email.validate()?;
    let payload = serde_json::to_string(email)?;
    anyhow::ensure!(payload.len() <= 60 * 1024, "queued email is too large");

    let job = toasty::create!(Job {
        deployment_id: None::<u64>,
        kind: "email",
        payload: Some(payload),
        error: None::<String>,
        status: "queued",
        attempts: 0_u32,
        available_at: crate::now(),
        created_at: crate::now(),
        finished_at: None::<i64>,
    })
    .exec(transaction)
    .await?;

    Ok(job.id)
}

impl Store {
    pub async fn enqueue_email(&self, email: &crate::mail::Email) -> anyhow::Result<u64> {
        let mut db = self.db.clone();
        let mut transaction = db.transaction().await?;
        let id = insert_email(&mut transaction, email).await?;

        transaction.commit().await?;
        Ok(id)
    }
}
