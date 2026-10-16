use super::{
    ApiError, State, auth,
    jsonapi::{self, ApiDocument, ApiInput, Input},
};
use axum::{
    extract::State as ExtractState,
    http::{HeaderMap, StatusCode, header},
};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetRequest {
    email: String,
}
impl Input for ResetRequest {
    const TYPE: &'static str = "password-resets";
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetConfirmation {
    token: String,
    password: String,
    confirmation: String,
}
impl Input for ResetConfirmation {
    const TYPE: &'static str = "password-resets";
}

pub async fn request_reset(
    ExtractState(state): ExtractState<State>,
    headers: HeaderMap,
    ApiInput(input): ApiInput<ResetRequest>,
) -> Result<(StatusCode, ApiDocument), ApiError> {
    if !auth::origin(&headers, &state) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Origin does not match this instance".into(),
        ));
    }
    crate::mail::validate_address(input.email.trim()).map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "Enter a valid email address".into(),
        )
    })?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    let result = async {
        let Ok(_permit) = state.login_slot.clone().try_acquire_owned() else {
            return Ok(());
        };
        let root = state.store.root_account().await?;
        let mut bytes = [0_u8; 32];
        rand::fill(&mut bytes);
        let token = hex::encode(bytes);
        let url = reqwest::Url::parse(&state.config.server.public_url)?;
        let secure = url.scheme() == "https"
            || (cfg!(debug_assertions)
                && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1")));
        let message = if state.config.mail.is_some() && secure {
            root.as_ref()
                .map(|root| {
                    crate::mail::SystemEmail::PasswordReset {
                        name: &root.name,
                        link: &format!(
                            "{}/reset-password#token={token}",
                            state.config.server.public_url.trim_end_matches('/')
                        ),
                    }
                    .render(&root.email)
                })
                .transpose()?
        } else {
            None
        };
        state
            .store
            .request_password_reset(
                input.email.trim(),
                &state.config.server.session_secret,
                &token,
                message.as_ref(),
                crate::now(),
            )
            .await?;
        Ok::<(), anyhow::Error>(())
    }
    .await;
    tokio::time::sleep_until(deadline).await;
    result.map_err(ApiError::internal)?;
    Ok((
        StatusCode::ACCEPTED,
        jsonapi::one(jsonapi::resource(
            "password-resets",
            "request",
            json!({"message":"If an account matches that address, a reset link will be emailed shortly."}),
        )),
    ))
}

pub async fn confirm_reset(
    ExtractState(state): ExtractState<State>,
    headers: HeaderMap,
    ApiInput(input): ApiInput<ResetConfirmation>,
) -> Result<(HeaderMap, StatusCode), ApiError> {
    if !auth::origin(&headers, &state) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Origin does not match this instance".into(),
        ));
    }
    if input.password.chars().count() < 15
        || input.password.len() > 1024
        || input.password != input.confirmation
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Use at least 15 characters, at most 1024 bytes, and matching passwords".into(),
        ));
    }
    let invalid = || {
        ApiError(
            StatusCode::BAD_REQUEST,
            "This reset link is invalid or expired. Request a new one.".into(),
        )
    };
    if input.token.len() != 64 || !input.token.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    let _permit = state
        .login_slot
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError(StatusCode::TOO_MANY_REQUESTS, "Try again shortly".into()))?;
    if !state
        .store
        .password_reset_valid(
            &state.config.server.session_secret,
            &input.token,
            crate::now(),
        )
        .await
        .map_err(ApiError::internal)?
    {
        return Err(invalid());
    }
    let hash = tokio::task::spawn_blocking(move || {
        use argon2::PasswordHasher;
        argon2::Argon2::default()
            .hash_password(input.password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(|_| anyhow::anyhow!("password hashing failed"))
    })
    .await
    .map_err(|e| ApiError::internal(e.into()))?
    .map_err(ApiError::internal)?;
    let user = state
        .store
        .root_account()
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(invalid)?;
    let notification = crate::mail::SystemEmail::PasswordChanged {
        name: &user.name,
        link: &state.config.server.public_url,
    }
    .render(&user.email)
    .map_err(ApiError::internal)?;
    if !state
        .store
        .consume_password_reset(
            &state.config.server.session_secret,
            &input.token,
            &hash,
            &notification,
            crate::now(),
        )
        .await
        .map_err(ApiError::internal)?
    {
        return Err(invalid());
    }
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::SET_COOKIE,
        auth::cookie(&state, "", 0).parse().expect("cookie"),
    );
    Ok((response_headers, StatusCode::NO_CONTENT))
}
