use anyhow::{Context, ensure};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub server: Server,
    pub database: Database,
    pub providers: Providers,
    pub worker: Worker,
    pub mail: Option<Mail>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub listen: std::net::SocketAddr,
    pub public_url: String,
    pub assets_root: PathBuf,
    pub state_root: PathBuf,
    pub session_secret: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Database {
    pub url: String,
    pub test_url: Option<String>,
}

impl Database {
    pub fn load() -> anyhow::Result<Self> {
        Self::with_path(config_path())
    }

    pub fn with_path(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        // Database tools need only this section, even before runtime setup exists.
        #[derive(Deserialize)]
        struct Document {
            database: Database,
        }

        let contents = std::fs::read_to_string(path).context("read database configuration")?;
        let document: Document =
            toml::from_str(&contents).context("parse database configuration")?;

        Ok(document.database)
    }

    pub fn connection(&self, test: bool) -> anyhow::Result<&str> {
        if !test {
            return Ok(&self.url);
        }

        let test_url = self
            .test_url
            .as_deref()
            .context("--test requires database.test_url")?;
        ensure!(
            test_url != self.url,
            "database.test_url must differ from database.url"
        );

        Ok(test_url)
    }
}

fn config_path() -> PathBuf {
    std::env::var_os("DEPLOYER_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.toml"))
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Providers {
    #[serde(default)]
    pub current: ProviderKind,
    pub github: Option<Github>,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    #[default]
    Github,
}

impl Providers {
    pub fn github(&self) -> anyhow::Result<&Github> {
        self.github
            .as_ref()
            .context("providers.github configuration is required")
    }

    fn validate(&self) -> anyhow::Result<()> {
        match self.current {
            ProviderKind::Github => {
                ensure!(
                    self.github()?.webhook_secret.len() >= 32,
                    "webhook_secret must have at least 32 characters"
                );
            }
        }

        Ok(())
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Github {
    pub app_id: String,
    pub installation_id: u64,
    pub private_key: PathBuf,
    pub webhook_secret: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Worker {
    pub apps_root: PathBuf,
    pub nginx_helper: PathBuf,
    #[serde(default = "default_https_helper")]
    pub https_helper: PathBuf,
    pub max_artifact_bytes: u64,
    pub max_extracted_bytes: u64,
    pub poll_seconds: u64,
}

fn default_https_helper() -> PathBuf {
    "/usr/local/libexec/deployer-acme".into()
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mail {
    pub from_email: String,
    #[serde(default = "default_mail_name")]
    pub from_name: String,
    pub smtp: Smtp,
}

fn default_mail_name() -> String {
    "Deployer".into()
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Smtp {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub security: SmtpSecurity,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmtpSecurity {
    #[default]
    Starttls,
    Tls,
    None,
}

impl Mail {
    pub fn validate(&self) -> anyhow::Result<()> {
        crate::mail::validate_address(&self.from_email)?;
        ensure!(
            !self.from_name.trim().is_empty()
                && self.from_name.len() <= 128
                && !self.from_name.chars().any(char::is_control),
            "invalid mail.from_name"
        );
        ensure!(
            !self.smtp.host.is_empty()
                && self.smtp.host.len() <= 253
                && !self.smtp.host.chars().any(char::is_whitespace)
                && self.smtp.port > 0,
            "invalid SMTP host or port"
        );
        ensure!(
            self.smtp.username.is_some() == self.smtp.password.is_some(),
            "SMTP username and password must be configured together"
        );

        if matches!(self.smtp.security, SmtpSecurity::None) {
            ensure!(
                self.smtp.host == "localhost"
                    || self
                        .smtp
                        .host
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback()),
                "unencrypted SMTP is supported only for a local relay"
            );
        }

        Ok(())
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config: Self = toml::from_str(
            &std::fs::read_to_string(config_path()).context("read Deployer configuration")?,
        )?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let url = reqwest::Url::parse(&self.server.public_url)?;
        ensure!(
            matches!(url.scheme(), "https" | "http") && url.host_str().is_some(),
            "public_url must be an HTTP origin"
        );
        ensure!(
            url.path() == "/"
                && url.query().is_none()
                && url.fragment().is_none()
                && url.username().is_empty()
                && url.password().is_none(),
            "public_url must contain only an origin"
        );
        ensure!(
            cfg!(debug_assertions) || url.scheme() == "https",
            "production public_url requires HTTPS"
        );
        ensure!(
            self.server.session_secret.len() >= 32,
            "session_secret must have at least 32 characters"
        );
        self.providers.validate()?;

        if let Some(mail) = &self.mail {
            mail.validate()?;
        }

        ensure!(
            self.worker.poll_seconds > 0
                && self.worker.max_artifact_bytes > 0
                && self.worker.max_extracted_bytes > 0,
            "worker limits must be positive"
        );
        Ok(())
    }

    pub fn frontend_directory(&self) -> PathBuf {
        self.server.assets_root.join(crate::VERSION)
    }
}

#[cfg(test)]
mod tests {
    use super::Database;

    #[test]
    fn database_tools_load_without_runtime_configuration() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        std::fs::write(&path, "[database]\nurl = 'mysql://localhost/deployer'\n").unwrap();

        let database = Database::with_path(&path).unwrap();
        assert_eq!(
            database.connection(false).unwrap(),
            "mysql://localhost/deployer"
        );

        // Unfinished runtime settings must not prevent schema tooling from running.
        std::fs::write(
            &path,
            "[database]\nurl = 'mysql://localhost/deployer'\n[server]\nlisten = 42\n",
        )
        .unwrap();

        assert!(Database::with_path(&path).is_ok());
    }

    #[test]
    fn test_database_selection_requires_a_separate_explicit_url() {
        let mut database = Database {
            url: "mysql://localhost/deployer".into(),
            test_url: None,
        };

        assert!(database.connection(true).is_err());

        database.test_url = Some(database.url.clone());
        assert!(database.connection(true).is_err());

        database.test_url = Some("mysql://localhost/deployer_test".into());
        assert_eq!(
            database.connection(true).unwrap(),
            "mysql://localhost/deployer_test"
        );
        assert_eq!(
            database.connection(false).unwrap(),
            "mysql://localhost/deployer"
        );
    }
}
