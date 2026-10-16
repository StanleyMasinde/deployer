use super::{ApiError, State};
use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};

pub fn origin(headers: &HeaderMap, state: &State) -> bool {
    headers
        .get("origin")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|o| o == state.config.server.public_url.trim_end_matches('/'))
}

pub async fn protect(
    axum::extract::State(state): axum::extract::State<State>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = request
        .headers()
        .get("cookie")
        .and_then(|h| h.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .filter_map(|p| p.trim().split_once('='))
                .find(|(k, _)| *k == "deployer_session")
                .map(|(_, v)| v)
        });
    let valid = match token {
        Some(token) => state
            .store
            .valid_session(&state.config.server.session_secret, token, crate::now())
            .await
            .map_err(ApiError::internal)?,
        None => false,
    };
    if !valid {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "Sign in to continue".into(),
        ));
    }

    if !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) && !origin(request.headers(), &state)
    {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Origin does not match this instance".into(),
        ));
    }

    Ok(next.run(request).await)
}

pub fn cookie(state: &State, value: &str, max_age: u32) -> String {
    format!(
        "deployer_session={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}{}",
        if state.config.server.public_url.starts_with("https://") {
            "; Secure"
        } else {
            ""
        }
    )
}
