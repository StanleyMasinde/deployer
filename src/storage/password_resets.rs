use super::{
    Store,
    models::{PasswordReset, User},
};
use crate::application::sessions::{reset_digest, reset_matches};
impl Store {
    pub async fn valid_session(&self, secret: &str, token: &str, now: i64) -> anyhow::Result<bool> {
        let Some(user) = self.root_account().await? else {
            return Ok(false);
        };
        Ok(crate::application::sessions::valid(
            &crate::application::sessions::credential_key(secret, &user.password_hash),
            token,
            now,
        ))
    }
    pub async fn request_password_reset(
        &self,
        email: &str,
        secret: &str,
        token: &str,
        message: Option<&crate::mail::Email>,
        now: i64,
    ) -> anyhow::Result<bool> {
        let mut db = self.db.clone();
        let mut tx = db.transaction().await?;
        toasty::sql::statement("UPDATE users SET id = id WHERE id = 1")
            .exec(&mut tx)
            .await?;
        let user = User::filter_by_id(1_u64).first().exec(&mut tx).await?;
        let existing = PasswordReset::filter_by_id(1_u64)
            .first()
            .exec(&mut tx)
            .await?;
        let allowed = existing
            .as_ref()
            .is_none_or(|reset| reset.created_at <= now - 60);
        if !allowed
            || !user.is_some_and(|user| user.email.eq_ignore_ascii_case(email.trim()))
            || message.is_none()
        {
            return Ok(false);
        }
        toasty::sql::statement("INSERT INTO password_resets (id, token_hash, created_at, expires_at, used) VALUES (1, ?, ?, ?, FALSE) ON DUPLICATE KEY UPDATE token_hash = VALUES(token_hash), created_at = VALUES(created_at), expires_at = VALUES(expires_at), used = FALSE")
            .bind(reset_digest(secret,token)).bind(now).bind(now+1800).exec(&mut tx).await?;
        super::jobs::insert_email(&mut tx, message.expect("checked above")).await?;
        tx.commit().await?;
        Ok(true)
    }
    pub async fn password_reset_valid(
        &self,
        secret: &str,
        token: &str,
        now: i64,
    ) -> anyhow::Result<bool> {
        Ok(PasswordReset::filter_by_id(1_u64)
            .first()
            .exec(&mut self.db.clone())
            .await?
            .is_some_and(|reset| {
                !reset.used
                    && reset.expires_at > now
                    && reset_matches(secret, token, &reset.token_hash)
            }))
    }
    pub async fn consume_password_reset(
        &self,
        secret: &str,
        token: &str,
        hash: &str,
        notification: &crate::mail::Email,
        now: i64,
    ) -> anyhow::Result<bool> {
        let mut db = self.db.clone();
        let mut tx = db.transaction().await?;
        toasty::sql::statement("UPDATE users SET id = id WHERE id = 1")
            .exec(&mut tx)
            .await?;
        let reset = PasswordReset::filter_by_id(1_u64)
            .first()
            .exec(&mut tx)
            .await?;
        let Some(reset) = reset else {
            return Ok(false);
        };
        if reset.used || reset.expires_at <= now || !reset_matches(secret, token, &reset.token_hash)
        {
            return Ok(false);
        }
        User::filter_by_id(1_u64)
            .update()
            .password_hash(hash)
            .exec(&mut tx)
            .await?;
        PasswordReset::filter_by_id(1_u64)
            .update()
            .used(true)
            .token_hash(String::new())
            .exec(&mut tx)
            .await?;
        super::jobs::insert_email(&mut tx, notification).await?;
        tx.commit().await?;
        self.notify_changes();
        Ok(true)
    }
}
