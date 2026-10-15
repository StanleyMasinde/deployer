pub mod webhook;
pub mod workflow;

use crate::{
    config::Github as Config,
    domain::{App, Artifact, BuildCompleted, Repository},
    providers::SourceProvider,
};
use anyhow::{Context, ensure};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::{io::AsyncWriteExt, sync::Mutex};

#[derive(Deserialize)]
struct InstallationRepository {
    full_name: String,
    default_branch: String,
    pushed_at: Option<String>,
}

fn recent_repositories(mut repos: Vec<InstallationRepository>) -> Vec<Repository> {
    // GitHub returns UTC timestamps in ISO 8601 format, so lexical order is chronological.
    repos.sort_by(|a, b| {
        b.pushed_at
            .cmp(&a.pushed_at)
            .then_with(|| a.full_name.cmp(&b.full_name))
    });

    repos
        .into_iter()
        .map(|repo| Repository {
            full_name: repo.full_name,
            default_branch: repo.default_branch,
        })
        .collect()
}

pub struct Github {
    config: Config,
    key: jsonwebtoken::EncodingKey,
    client: reqwest::Client,
    download_client: reqwest::Client,
    token: Mutex<Option<(String, i64)>>,
}

impl Github {
    pub fn new(config: Config) -> anyhow::Result<Arc<Self>> {
        let key = jsonwebtoken::EncodingKey::from_rsa_pem(
            &std::fs::read(&config.private_key).context("read GitHub App private key")?,
        )?;
        let client = reqwest::Client::builder()
            .user_agent(concat!("deployer/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let download_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .https_only(true)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;
        Ok(Arc::new(Self {
            config,
            key,
            client,
            download_client,
            token: Mutex::new(None),
        }))
    }

    async fn token(&self) -> anyhow::Result<String> {
        let mut cached = self.token.lock().await;
        if let Some((token, expires)) = cached.as_ref()
            && *expires > crate::now()
        {
            return Ok(token.clone());
        }

        #[derive(Serialize)]
        struct Claims<'a> {
            iat: i64,
            exp: i64,
            iss: &'a str,
        }

        let jwt = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
            &Claims {
                iat: crate::now() - 60,
                exp: crate::now() + 540,
                iss: &self.config.app_id,
            },
            &self.key,
        )?;
        let response = self
            .client
            .post(format!(
                "https://api.github.com/app/installations/{}/access_tokens",
                self.config.installation_id
            ))
            .bearer_auth(jwt)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;
        #[derive(Deserialize)]
        struct Token {
            token: String,
        }

        let token: Token = response.error_for_status()?.json().await?;
        *cached = Some((token.token.clone(), crate::now() + 3000));
        Ok(token.token)
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> anyhow::Result<reqwest::RequestBuilder> {
        Ok(self
            .client
            .request(method, format!("https://api.github.com{path}"))
            .bearer_auth(self.token().await?)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28"))
    }

    async fn get(&self, path: &str) -> anyhow::Result<Value> {
        Ok(self
            .request(reqwest::Method::GET, path)
            .await?
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    async fn post(&self, path: &str, body: Value) -> anyhow::Result<Value> {
        Ok(self
            .request(reqwest::Method::POST, path)
            .await?
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}

impl SourceProvider for Github {
    fn name(&self) -> &'static str {
        "github"
    }

    async fn repositories(&self) -> anyhow::Result<Vec<Repository>> {
        let mut repos = Vec::new();
        for page in 1..=100 {
            let value = self
                .get(&format!(
                    "/installation/repositories?per_page=100&page={page}"
                ))
                .await?;

            let rows = value["repositories"]
                .as_array()
                .context("GitHub repositories missing")?;
            let count = rows.len();
            repos.extend(
                serde_json::from_value::<Vec<InstallationRepository>>(
                    value["repositories"].clone(),
                )
                .context("parse GitHub repositories")?,
            );

            if count < 100 {
                return Ok(recent_repositories(repos));
            }
        }

        anyhow::bail!("installation exceeds the supported repository listing limit");
    }

    async fn open_workflow_pr(&self, app: &App) -> anyhow::Result<String> {
        let prefix = format!("/repos/{}", app.repository);
        let branch = format!("deployer/app-{}", app.id);
        let owner = app
            .repository
            .split('/')
            .next()
            .context("repository owner")?;
        let mut query = reqwest::Url::parse("https://api.github.com")?;
        query
            .query_pairs_mut()
            .append_pair("head", &format!("{owner}:{branch}"))
            .append_pair("base", &app.branch)
            .append_pair("state", "open");
        let prs = self
            .get(&format!(
                "{prefix}/pulls?{}",
                query.query().unwrap_or_default()
            ))
            .await?;
        let open_pr = prs
            .as_array()
            .and_then(|a| a.first())
            .and_then(|pr| pr["html_url"].as_str())
            .map(str::to_owned);

        let reference = self
            .request(
                reqwest::Method::GET,
                &format!("{prefix}/git/ref/heads/{branch}"),
            )
            .await?
            .send()
            .await?;
        if reference.status() == reqwest::StatusCode::NOT_FOUND {
            let base = self
                .get(&format!("{prefix}/git/ref/heads/{}", app.branch))
                .await?;
            self.post(
                &format!("{prefix}/git/refs"),
                json!({"ref":format!("refs/heads/{branch}"),"sha":base["object"]["sha"]}),
            )
            .await?;
        } else {
            reference.error_for_status()?;
        }

        let content_path = format!("{prefix}/contents/{}", workflow::PATH);
        let existing = self
            .request(reqwest::Method::GET, &content_path)
            .await?
            .query(&[("ref", &branch)])
            .send()
            .await?;

        let mut body = json!({"message":"ci: build static app for Deployer", "branch":branch, "content":base64::engine::general_purpose::STANDARD.encode(workflow::render(app))});
        if existing.status() != reqwest::StatusCode::NOT_FOUND {
            body["sha"] = existing.error_for_status()?.json::<Value>().await?["sha"].clone();
        }
        self.request(reqwest::Method::PUT, &content_path)
            .await?
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        if let Some(url) = open_pr {
            return Ok(url);
        }
        let pr = self.post(&format!("{prefix}/pulls"),json!({"title":"Build and deploy static app with Deployer", "head":branch,"base":app.branch,"body":"Builds run on GitHub Actions. Deployer downloads the successful artifact after a signed workflow_run webhook. Review the build commands and output directory before merging."})).await?;

        Ok(pr["html_url"].as_str().context("PR URL")?.into())
    }

    async fn artifact(&self, app: &App, build: &BuildCompleted) -> anyhow::Result<Artifact> {
        let prefix = format!("/repos/{}", app.repository);
        let run = self
            .get(&format!("{prefix}/actions/runs/{}", build.run_id))
            .await?;
        ensure!(
            run["conclusion"] == "success"
                && run["status"] == "completed"
                && run["head_sha"] == build.commit_sha
                && run["head_branch"] == app.branch
                && run["run_attempt"].as_u64() == Some(build.attempt.into())
                && run["path"] == workflow::PATH
                && matches!(run["event"].as_str(), Some("push" | "workflow_dispatch"))
                && run["head_repository"]["full_name"]
                    .as_str()
                    .is_some_and(|r| r.eq_ignore_ascii_case(&app.repository)),
            "workflow run does not match the trusted deployment"
        );
        let list = self
            .get(&format!(
                "{prefix}/actions/runs/{}/artifacts?name={}&per_page=100",
                build.run_id,
                workflow::ARTIFACT
            ))
            .await?;

        let artifacts = list["artifacts"].as_array().context("artifacts missing")?;
        let rows: Vec<_> = artifacts
            .iter()
            .filter(|a| a["name"] == workflow::ARTIFACT && a["expired"] == false)
            .collect();
        ensure!(
            rows.len() == 1,
            "expected one unexpired deployer-app artifact"
        );
        let artifact = rows[0];
        let digest = artifact["digest"]
            .as_str()
            .context("artifact digest missing")?;
        ensure!(
            digest.starts_with("sha256:") && digest.len() == 71,
            "invalid artifact digest"
        );
        Ok(Artifact {
            id: artifact["id"].as_u64().context("artifact ID")?,
            digest: digest.into(),
        })
    }

    async fn download(
        &self,
        app: &App,
        artifact: &Artifact,
        path: &Path,
        limit: u64,
    ) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::GET,
                &format!(
                    "/repos/{}/actions/artifacts/{}/zip",
                    app.repository, artifact.id
                ),
            )
            .await?
            .send()
            .await?;
        ensure!(
            response.status() == reqwest::StatusCode::FOUND,
            "GitHub did not return an artifact redirect"
        );
        let url = reqwest::Url::parse(
            response
                .headers()
                .get(reqwest::header::LOCATION)
                .context("missing download location")?
                .to_str()?,
        )?;
        ensure!(url.scheme() == "https", "artifact URL must use HTTPS");
        // The signed URL is fetched without installation credentials.
        let mut response = self
            .download_client
            .get(url)
            .send()
            .await?
            .error_for_status()?;
        ensure!(
            response.content_length().is_none_or(|n| n <= limit),
            "artifact exceeds download limit"
        );
        let mut file = tokio::fs::File::create(path).await?;

        let mut hash = Sha256::new();
        let mut size = 0u64;
        while let Some(chunk) = response.chunk().await? {
            size = size
                .checked_add(chunk.len() as u64)
                .context("artifact length overflow")?;
            ensure!(size <= limit, "artifact exceeds download limit");
            hash.update(&chunk);
            file.write_all(&chunk).await?;
        }
        file.sync_all().await?;
        ensure!(
            format!("sha256:{}", hex::encode(hash.finalize())) == artifact.digest,
            "artifact checksum mismatch"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{InstallationRepository, recent_repositories};
    use serde_json::json;

    #[test]
    fn repositories_sort_by_last_push_across_pages_with_missing_dates_last() {
        let pages = [
            json!([
                {"full_name":"owner/old","default_branch":"main","pushed_at":"2025-01-01T00:00:00Z"},
                {"full_name":"owner/empty-z","default_branch":"main","pushed_at":null},
                {"full_name":"owner/recent-z","default_branch":"develop","pushed_at":"2026-10-16T12:00:00Z"}
            ]),
            json!([
                {"full_name":"owner/recent-a","default_branch":"production","pushed_at":"2026-10-16T12:00:00Z"},
                {"full_name":"owner/newest","default_branch":"main","pushed_at":"2026-10-16T13:00:00Z"},
                {"full_name":"owner/empty-a","default_branch":"main"}
            ]),
        ];
        let repos = pages
            .into_iter()
            .flat_map(|page| serde_json::from_value::<Vec<InstallationRepository>>(page).unwrap())
            .collect();
        let sorted = recent_repositories(repos);
        assert_eq!(
            sorted
                .iter()
                .map(|repo| repo.full_name.as_str())
                .collect::<Vec<_>>(),
            [
                "owner/newest",
                "owner/recent-a",
                "owner/recent-z",
                "owner/old",
                "owner/empty-a",
                "owner/empty-z"
            ]
        );
        assert_eq!(sorted[1].default_branch, "production");
        assert_eq!(sorted[2].default_branch, "develop");
    }
}
