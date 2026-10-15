use crate::storage::Store;
use anyhow::{Context, ensure};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use tokio::sync::Mutex;

pub struct CreateRoot {
    pub code: String,
    pub name: String,
    pub email: String,
    pub password: String,
}

pub struct Setup {
    code: Mutex<Option<String>>,
    path: PathBuf,
}

impl Setup {
    pub async fn load(store: &Store, state_root: &Path) -> anyhow::Result<Self> {
        let path = state_root.join("setup-code");

        if store.root_account().await?.is_some() {
            return Ok(Self {
                code: Mutex::new(None),
                path,
            });
        }

        std::fs::create_dir_all(state_root)?;

        let code = if path.exists() {
            std::fs::read_to_string(&path)?
        } else {
            let mut bytes = [0_u8; 32];
            rand::fill(&mut bytes);
            let code = hex::encode(bytes);
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);

            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }

            match options.open(&path) {
                Ok(mut file) => {
                    file.write_all(code.as_bytes())?;
                    file.sync_all()?;
                    code
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::fs::read_to_string(&path)?
                }
                Err(error) => return Err(error.into()),
            }
        };

        ensure!(code.len() == 64, "invalid setup code file");
        tracing::info!(path = %path.display(), "No root account exists. Open /setup and read the one-time code from this file");

        Ok(Self {
            code: Mutex::new(Some(code)),
            path,
        })
    }

    pub async fn create_root(
        &self,
        store: &Store,
        input: CreateRoot,
        welcome_link: Option<&str>,
    ) -> anyhow::Result<()> {
        let CreateRoot {
            code,
            name,
            email,
            password,
        } = input;
        crate::mail::validate_address(&email)?;

        let name = name.trim();
        ensure!(
            (1..=128).contains(&name.chars().count()) && !name.chars().any(char::is_control),
            "name must contain 1–128 characters without control characters"
        );

        ensure!(
            (12..=1024).contains(&password.len()),
            "password must contain 12–1024 bytes"
        );
        ensure!(code.len() == 64, "invalid setup code");

        let mut current = self.code.lock().await;
        let expected = current.as_ref().context("root account already created")?;
        ensure!(
            store.root_account().await?.is_none(),
            "root account already created"
        );

        let mut expected_mac = Hmac::<Sha256>::new_from_slice(expected.as_bytes())?;
        expected_mac.update(b"deployer-root-setup");
        let signature = expected_mac.finalize().into_bytes();
        let mut actual_mac = Hmac::<Sha256>::new_from_slice(code.as_bytes())?;
        actual_mac.update(b"deployer-root-setup");
        actual_mac
            .verify_slice(&signature)
            .map_err(|_| anyhow::anyhow!("invalid setup code"))?;

        let hash = tokio::task::spawn_blocking(move || {
            use argon2::PasswordHasher;

            argon2::Argon2::default()
                .hash_password(password.as_bytes())
                .map(|hash| hash.to_string())
                .map_err(|_| anyhow::anyhow!("password hashing failed"))
        })
        .await??;

        let welcome = welcome_link
            .map(|link| crate::mail::SystemEmail::Welcome { name, link }.render(&email))
            .transpose()?;

        store
            .create_root(name, &email, &hash, welcome.as_ref())
            .await?;
        *current = None;

        if let Err(error) = std::fs::remove_file(&self.path) {
            tracing::warn!(%error,"could not remove consumed setup code file");
        }

        Ok(())
    }
}
