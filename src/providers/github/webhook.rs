use crate::domain::BuildCompleted;
use anyhow::ensure;
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;

pub fn verify(secret: &str, signature: &str, body: &[u8]) -> anyhow::Result<()> {
    let signature = signature
        .strip_prefix("sha256=")
        .ok_or_else(|| anyhow::anyhow!("missing SHA-256 signature"))?;
    let bytes = hex::decode(signature)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())?;
    mac.update(body);
    mac.verify_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("invalid webhook signature"))
}

#[derive(Deserialize)]
struct Payload {
    action: String,
    installation: Installation,
    repository: Repo,
    workflow_run: Run,
}

#[derive(Deserialize)]
struct Installation {
    id: u64,
}

#[derive(Deserialize)]
struct Repo {
    full_name: String,
}

#[derive(Deserialize)]
struct Run {
    id: u64,
    run_attempt: u32,
    head_branch: Option<String>,
    head_sha: String,
    event: String,
    conclusion: Option<String>,
    path: String,
    head_repository: Repo,
}

pub fn parse(body: &[u8], installation_id: u64) -> anyhow::Result<Option<BuildCompleted>> {
    let p: Payload = serde_json::from_slice(body)?;
    ensure!(
        p.installation.id == installation_id,
        "unexpected GitHub installation"
    );
    let run = p.workflow_run;
    if p.action != "completed"
        || run.conclusion.as_deref() != Some("success")
        || !matches!(run.event.as_str(), "push" | "workflow_dispatch")
        || run.path != super::workflow::PATH
        || !run
            .head_repository
            .full_name
            .eq_ignore_ascii_case(&p.repository.full_name)
    {
        return Ok(None);
    }

    ensure!(
        run.head_sha.len() == 40 && run.head_sha.bytes().all(|c| c.is_ascii_hexdigit()),
        "invalid commit SHA"
    );
    Ok(Some(BuildCompleted {
        repository: p.repository.full_name,
        branch: run.head_branch.unwrap_or_default(),
        run_id: run.id,
        attempt: run.run_attempt,
        commit_sha: run.head_sha,
    }))
}

#[derive(Debug)]
pub struct PullRequestUpdate {
    pub repository: String,
    pub url: String,
    pub state: &'static str,
    pub updated_at: String,
}

pub fn parse_pull_request(
    body: &[u8],
    installation_id: u64,
) -> anyhow::Result<Option<PullRequestUpdate>> {
    #[derive(Deserialize)]
    struct Pull {
        html_url: String,
        state: String,
        merged: bool,
        updated_at: String,
    }
    #[derive(Deserialize)]
    struct Event {
        action: String,
        installation: Installation,
        repository: Repo,
        pull_request: Pull,
    }
    let p: Event = serde_json::from_slice(body)?;
    ensure!(
        p.installation.id == installation_id,
        "unexpected GitHub installation"
    );
    if !matches!(
        p.action.as_str(),
        "opened" | "reopened" | "closed" | "synchronize" | "edited"
    ) {
        return Ok(None);
    }
    let state = if p.action == "closed" && p.pull_request.merged {
        "merged"
    } else if p.pull_request.state == "closed" {
        "closed"
    } else {
        "open"
    };
    ensure!(
        p.pull_request.updated_at.len() == 20 && p.pull_request.updated_at.ends_with('Z'),
        "invalid update timestamp"
    );
    Ok(Some(PullRequestUpdate {
        repository: p.repository.full_name,
        url: p.pull_request.html_url,
        state,
        updated_at: p.pull_request.updated_at,
    }))
}

pub fn parse_workflow_status(
    body: &[u8],
    installation_id: u64,
) -> anyhow::Result<Option<(BuildCompleted, &'static str, Option<String>)>> {
    let p: Payload = serde_json::from_slice(body)?;
    ensure!(
        p.installation.id == installation_id,
        "unexpected GitHub installation"
    );
    let run = p.workflow_run;
    if !matches!(run.event.as_str(), "push" | "workflow_dispatch")
        || run.path != super::workflow::PATH
        || !run
            .head_repository
            .full_name
            .eq_ignore_ascii_case(&p.repository.full_name)
    {
        return Ok(None);
    }
    let (status, error) = match p.action.as_str() {
        "requested" | "in_progress" => ("building", None),
        "completed" if run.conclusion.as_deref() == Some("success") => return Ok(None),
        "completed" => (
            "failed",
            Some(format!(
                "GitHub workflow concluded: {}",
                run.conclusion.as_deref().unwrap_or("unknown")
            )),
        ),
        _ => return Ok(None),
    };
    ensure!(
        run.head_sha.len() == 40 && run.head_sha.bytes().all(|c| c.is_ascii_hexdigit()),
        "invalid commit SHA"
    );
    Ok(Some((
        BuildCompleted {
            repository: p.repository.full_name,
            branch: run.head_branch.unwrap_or_default(),
            run_id: run.id,
            attempt: run.run_attempt,
            commit_sha: run.head_sha,
        },
        status,
        error,
    )))
}
