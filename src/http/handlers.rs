use super::jsonapi::{self, ApiDocument, ApiInput, Input};
use super::{ApiError, State};
use crate::providers::SourceProvider;
use axum::{
    Json,
    extract::{Path, Query, State as ExtractState},
    http::{HeaderMap, StatusCode, header},
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    email: String,
    password: String,
}

impl Input for Login {
    const TYPE: &'static str = "sessions";
}

pub async fn login(
    ExtractState(state): ExtractState<State>,
    headers: HeaderMap,
    ApiInput(input): ApiInput<Login>,
) -> Result<(HeaderMap, ApiDocument), ApiError> {
    if !super::auth::origin(&headers, &state) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Origin does not match this instance".into(),
        ));
    }

    if input.password.len() > 1024 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Password is too long".into(),
        ));
    }

    let permit = state
        .login_slot
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError(StatusCode::TOO_MANY_REQUESTS, "Try again shortly".into()))?;
    let root = state
        .store
        .root_account()
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| {
            ApiError(
                StatusCode::CONFLICT,
                "Create the root account at /setup".into(),
            )
        })?;
    let email_matches = root.email.eq_ignore_ascii_case(input.email.trim());
    let hash = root.password_hash;
    let valid = crate::application::sessions::verify_password(input.password, hash)
        .await
        .map_err(ApiError::internal)?;

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    drop(permit);
    if !valid || !email_matches {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "Incorrect email or password".into(),
        ));
    }

    let token = crate::application::sessions::issue(
        &state.config.server.session_secret,
        crate::now() + 86400,
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        super::auth::cookie(&state, &token, 86400)
            .parse()
            .expect("cookie"),
    );
    Ok((
        headers,
        jsonapi::one(jsonapi::resource(
            "sessions",
            "current",
            json!({"authenticated":true}),
        )),
    ))
}

pub async fn logout(ExtractState(state): ExtractState<State>) -> (HeaderMap, StatusCode) {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        super::auth::cookie(&state, "", 0).parse().expect("cookie"),
    );
    (headers, StatusCode::NO_CONTENT)
}

pub async fn session(ExtractState(state): ExtractState<State>) -> Result<ApiDocument, ApiError> {
    let user = state
        .store
        .root_account()
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "Sign in to continue".into()))?;
    Ok(jsonapi::one(jsonapi::resource(
        "sessions",
        "current",
        json!({"authenticated":true, "user":{"id":user.id,"name":user.name,"email":user.email}}),
    )))
}

pub async fn apps(ExtractState(state): ExtractState<State>) -> Result<ApiDocument, ApiError> {
    Ok(jsonapi::many(
        state
            .store
            .apps()
            .await
            .map_err(ApiError::internal)?
            .iter()
            .map(|s| jsonapi::resource("apps", s.id, s))
            .collect(),
    ))
}

pub async fn add_app(
    ExtractState(state): ExtractState<State>,
    ApiInput(input): ApiInput<crate::domain::NewApp>,
) -> Result<ApiDocument, ApiError> {
    input
        .validate()
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    let app = crate::application::apps::add(&state.store, state.current_provider.as_ref(), input)
        .await
        .map_err(ApiError::internal)?;

    Ok(jsonapi::one(jsonapi::resource("apps", app.id, app)))
}

fn app_error(error: anyhow::Error) -> ApiError {
    match error.downcast_ref::<crate::storage::AppError>() {
        Some(crate::storage::AppError::NotFound) => {
            ApiError(StatusCode::NOT_FOUND, error.to_string())
        }
        Some(_) => ApiError(StatusCode::CONFLICT, error.to_string()),
        None => ApiError::internal(error),
    }
}

pub async fn app(
    ExtractState(state): ExtractState<State>,
    Path(id): Path<u64>,
) -> Result<ApiDocument, ApiError> {
    let app = state.store.find_app(id).await.map_err(app_error)?;
    Ok(jsonapi::one(jsonapi::resource("apps", app.id, app)))
}

pub async fn update_app(
    ExtractState(state): ExtractState<State>,
    Path(id): Path<u64>,
    ApiInput(input): ApiInput<crate::domain::NewApp>,
) -> Result<ApiDocument, ApiError> {
    input
        .validate()
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    let app =
        crate::application::apps::update(&state.store, state.current_provider.as_ref(), id, input)
            .await
            .map_err(app_error)?;
    Ok(jsonapi::one(jsonapi::resource("apps", app.id, app)))
}

pub async fn delete_app(
    ExtractState(state): ExtractState<State>,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    state
        .store
        .request_app_deletion(id)
        .await
        .map_err(app_error)?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
pub struct DeploymentCursor {
    before: Option<u64>,
}

pub async fn app_deployments(
    ExtractState(state): ExtractState<State>,
    Path(id): Path<u64>,
    Query(cursor): Query<DeploymentCursor>,
) -> Result<ApiDocument, ApiError> {
    state.store.find_app(id).await.map_err(app_error)?;
    let rows = state
        .store
        .app_deployments_before(id, cursor.before)
        .await
        .map_err(ApiError::internal)?;
    Ok(jsonapi::many(
        rows.iter()
            .map(|d| jsonapi::resource("deployments", d.id, d))
            .collect(),
    ))
}

pub async fn workflow(
    ExtractState(state): ExtractState<State>,
    Path(id): Path<u64>,
) -> Result<ApiDocument, ApiError> {
    let app =
        crate::application::apps::create_pr(&state.store, state.current_provider.as_ref(), id)
            .await
            .map_err(app_error)?;

    Ok(jsonapi::one(jsonapi::resource("apps", app.id, app)))
}

pub async fn deployments(
    ExtractState(state): ExtractState<State>,
) -> Result<ApiDocument, ApiError> {
    Ok(jsonapi::many(
        state
            .store
            .deployments()
            .await
            .map_err(ApiError::internal)?
            .iter()
            .map(|d| jsonapi::resource("deployments", d.id, d))
            .collect(),
    ))
}

pub async fn repositories(
    ExtractState(state): ExtractState<State>,
) -> Result<ApiDocument, ApiError> {
    Ok(jsonapi::many(
        state
            .current_provider
            .repositories()
            .await
            .map_err(ApiError::internal)?
            .iter()
            .map(|r| jsonapi::resource("repositories", &r.full_name, r))
            .collect(),
    ))
}

pub async fn webhook(
    ExtractState(state): ExtractState<State>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<StatusCode, ApiError> {
    let github = state
        .config
        .providers
        .github()
        .map_err(|_| ApiError(StatusCode::NOT_FOUND, "GitHub is not configured".into()))?;

    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    crate::providers::github::webhook::verify(&github.webhook_secret, signature, &body)
        .map_err(|_| ApiError(StatusCode::UNAUTHORIZED, "Invalid webhook signature".into()))?;
    match headers.get("x-github-event").and_then(|h| h.to_str().ok()) {
        Some("ping") => return Ok(StatusCode::OK),
        Some("pull_request") => {
            if let Some(update) =
                crate::providers::github::webhook::parse_pull_request(&body, github.installation_id)
                    .map_err(|_| {
                        ApiError(StatusCode::BAD_REQUEST, "Invalid pull request event".into())
                    })?
            {
                state
                    .store
                    .update_workflow_pr(&update)
                    .await
                    .map_err(ApiError::internal)?;
            }
            return Ok(StatusCode::ACCEPTED);
        }
        Some("workflow_run") => {}
        _ => return Ok(StatusCode::NO_CONTENT),
    }

    if let Some(build) = crate::providers::github::webhook::parse(&body, github.installation_id)
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid workflow event".into()))?
    {
        crate::application::webhooks::completed(&state.store, "github", build)
            .await
            .map_err(ApiError::internal)?;
    }

    Ok(StatusCode::ACCEPTED)
}

pub async fn readiness(ExtractState(state): ExtractState<State>) -> Result<Json<Value>, ApiError> {
    state
        .store
        .root_account()
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(json!({"status":"ready","version":crate::VERSION})))
}

pub async fn setup_status(
    ExtractState(state): ExtractState<State>,
) -> Result<ApiDocument, ApiError> {
    let required = state
        .store
        .root_account()
        .await
        .map_err(ApiError::internal)?
        .is_none();

    Ok(jsonapi::one(jsonapi::resource(
        "setup",
        "current",
        json!({"required": required}),
    )))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootSetup {
    code: String,
    name: String,
    email: String,
    password: String,
}

impl Input for RootSetup {
    const TYPE: &'static str = "users";
}

pub async fn setup_root(
    ExtractState(state): ExtractState<State>,
    headers: HeaderMap,
    ApiInput(input): ApiInput<RootSetup>,
) -> Result<(HeaderMap, ApiDocument), ApiError> {
    if !super::auth::origin(&headers, &state) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Origin does not match this instance".into(),
        ));
    }

    let _permit = state
        .login_slot
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError(StatusCode::TOO_MANY_REQUESTS, "Try again shortly".into()))?;

    state
        .setup
        .create_root(
            &state.store,
            crate::application::setup::CreateRoot {
                code: input.code,
                name: input.name.clone(),
                email: input.email.clone(),
                password: input.password,
            },
            state
                .config
                .mail
                .as_ref()
                .map(|_| state.config.server.public_url.as_str()),
        )
        .await
        .map_err(|error| ApiError(StatusCode::BAD_REQUEST, error.to_string()))?;

    let token = crate::application::sessions::issue(
        &state.config.server.session_secret,
        crate::now() + 86400,
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        super::auth::cookie(&state, &token, 86400)
            .parse()
            .expect("cookie"),
    );

    Ok((
        headers,
        jsonapi::one(jsonapi::resource(
            "users",
            "1",
            json!({"name":input.name.trim(),"email":input.email}),
        )),
    ))
}

pub async fn enable_https(
    ExtractState(state): ExtractState<State>,
    Path(id): Path<u64>,
) -> Result<ApiDocument, ApiError> {
    let app = state.store.request_https(id).await.map_err(app_error)?;
    Ok(jsonapi::one(jsonapi::resource("apps", app.id, app)))
}

pub async fn deployment(
    ExtractState(state): ExtractState<State>,
    Path(id): Path<u64>,
) -> Result<ApiDocument, ApiError> {
    let deployment = state
        .store
        .deployment_optional(id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Deployment not found".into()))?;
    Ok(jsonapi::one(jsonapi::resource(
        "deployments",
        deployment.id,
        &deployment,
    )))
}
