use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

pub fn issue(secret: &str, expires: i64) -> String {
    let payload = expires.to_string();
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    format!("{payload}.{}", hex::encode(mac.finalize().into_bytes()))
}

pub fn valid(secret: &str, token: &str, now: i64) -> bool {
    let Some((expires, signature)) = token.split_once('.') else {
        return false;
    };
    let Ok(time) = expires.parse::<i64>() else {
        return false;
    };
    if time <= now || time > now + 86400 {
        return false;
    }

    let Ok(bytes) = hex::decode(signature) else {
        return false;
    };
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(expires.as_bytes());
    mac.verify_slice(&bytes).is_ok()
}

pub async fn verify_password(password: String, hash: String) -> anyhow::Result<bool> {
    tokio::task::spawn_blocking(move || {
        use argon2::PasswordVerifier;

        let hash = argon2::PasswordHash::new(&hash)
            .map_err(|_| anyhow::anyhow!("invalid configured password hash"))?;

        Ok(argon2::Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok())
    })
    .await?
}
